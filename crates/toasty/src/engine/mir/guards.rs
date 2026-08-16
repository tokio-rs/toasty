//! Guard annotation: marks pure nodes whose output is only consumed when
//! another node produced at least one row.
//!
//! A guarded node is skipped at execution time when its condition node's
//! output is empty (see `exec::If`). The analysis is conservative — a node
//! with any consumer whose reference is observable under the failed condition
//! gets no guard and always executes. Two rules:
//!
//! 1. A node referenced only inside the body of an `Eval` `map` over node
//!    `X`'s rows is guarded by `non_empty(X)` — mapping zero rows never
//!    evaluates the body, so the node's output is unobservable when `X` is
//!    empty.
//! 2. A pure node whose consumers all carry guard `non_empty(X)` inherits
//!    that guard.
//!
//! The pass runs over the finished graph at `LogicalPlan::new` — not
//! incrementally during statement planning, where a node's consumer set is
//! still growing and a consumer created after an annotation could invalidate
//! it.

use std::collections::HashMap;

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit};

use super::{Cond, NodeId, Operation, Store};

pub(super) fn annotate_guards(store: &mut Store, execution_order: &[NodeId], completion: NodeId) {
    // Execution position; a guard's condition node must run before the nodes
    // it guards so the condition variable exists when the branch is taken.
    let position: HashMap<NodeId, usize> = execution_order
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    // Consumers are the reverse of the value edges — read from operation
    // inputs, never from `deps`, so an ordering-only edge never looks like a
    // consumer.
    let mut consumers: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for &id in execution_order {
        for input in store[id].op.inputs() {
            consumers.entry(input).or_default().push(id);
        }
    }

    // Rule 1: for each `Eval` mapping over one input's rows, the inputs
    // referenced only inside the map body. Keyed by (eval, input) -> the
    // mapped-over node.
    let mut body_only: HashMap<(NodeId, NodeId), NodeId> = HashMap::new();
    for &id in execution_order {
        let Operation::Eval(eval) = &store[id].op else {
            continue;
        };
        let Some((base_pos, body_positions)) = map_body_only_positions(eval.eval.expr()) else {
            continue;
        };
        let Some(&base_node) = eval.inputs.get_index(base_pos) else {
            continue;
        };
        for pos in body_positions {
            let Some(&input_node) = eval.inputs.get_index(pos) else {
                continue;
            };
            if input_node != base_node {
                body_only.insert((id, input_node), base_node);
            }
        }
    }

    // Consumers appear after producers in the topological order, so one pass
    // in reverse order decides every consumer's guard before its producers
    // read it (rule 2's closure).
    for &id in execution_order.iter().rev() {
        // Effects always run; the completion node is consumed by the engine.
        // A planner-assigned guard (`Cond::Expr`) is left untouched.
        if store[id].op.is_effectful() || id == completion || store[id].guard.is_some() {
            continue;
        }

        let Some(consumer_list) = consumers.get(&id) else {
            continue;
        };

        let mut guard = None;
        for &consumer in consumer_list {
            // The consumption is unobservable under `non_empty(x)` failing
            // when it sits in a map body over `x`'s rows (rule 1) or the
            // consumer itself is guarded on `x` (rule 2). The analysis
            // traffics in the condition's subject node; the `Cond` is built
            // at the write below. Planner-assigned `Cond::Expr` guards do
            // not propagate: their else-arm placeholder is an observed
            // value, not proven unobservable, so producers feeding them
            // must still execute.
            let candidate =
                body_only
                    .get(&(consumer, id))
                    .copied()
                    .or_else(|| match &store[consumer].guard {
                        Some(Cond::NonEmpty(x)) => Some(*x),
                        Some(Cond::Expr { .. }) | None => None,
                    });

            match (guard, candidate) {
                (None, Some(x)) => guard = Some(x),
                (Some(prev), Some(x)) if prev == x => {}
                _ => {
                    guard = None;
                    break;
                }
            }
        }

        if let Some(x) = guard
            && position[&x] < position[&id]
        {
            store[id].guard = Some(Cond::NonEmpty(x));
        }
    }
}

/// Matches an `Eval` function of the form `map(arg(b), body)` and returns
/// `(b, positions)` where `positions` are the statement-level input positions
/// referenced inside the map body (excluding `b`). The base is exactly one
/// arg, so any other referenced input is body-only.
fn map_body_only_positions(expr: &stmt::Expr) -> Option<(usize, IndexSet<usize>)> {
    let stmt::Expr::Map(map) = expr else {
        return None;
    };
    let stmt::Expr::Arg(base_arg) = &*map.base else {
        return None;
    };
    if base_arg.nesting != 0 {
        return None;
    }

    // The body sits inside the map's element scope, so an arg resolves to
    // the Eval's inputs when `nesting == scope_depth`.
    let mut positions = IndexSet::new();
    visit::walk_expr_scoped(&map.map, 1, |expr, scope_depth| {
        if let stmt::Expr::Arg(arg) = expr
            && arg.nesting == scope_depth
        {
            positions.insert(arg.position);
        }
        true
    });
    positions.shift_remove(&base_arg.position);

    Some((base_arg.position, positions))
}
