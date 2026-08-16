//! Attaches guards to plan nodes. A guard is a condition on a node; when the
//! condition is false at execution time, the node is skipped (see `exec::If`).
//!
//! # Why guards exist
//!
//! A plan often contains nodes whose output only matters when an earlier node
//! returned rows. Consider loading todos together with each todo's user:
//!
//! ```text
//! n0: query todos
//! n1: extract the user ids from n0's rows
//! n2: query users whose id is in n1
//! n3: map over n0's rows, attaching each todo's user from n2
//! ```
//!
//! When n0 returns no rows, the map in n3 iterates zero times, so nothing
//! ever reads n2 — the user query is a wasted database round-trip. This pass
//! guards n1 and n2 with the condition "n0 returned at least one row"
//! ([`Cond::NonEmpty`]`(n0)`). At execution time, when n0 comes back empty,
//! n1 and n2 are skipped.
//!
//! # The two rules
//!
//! A node may be skipped only when nothing reads its output. Two rules prove
//! that:
//!
//! 1. A node referenced only inside the body of an `Eval` `map` over node
//!    X's rows gets guard `non_empty(X)`. Mapping zero rows never runs the
//!    body, so the node's output goes unread whenever X is empty. This is
//!    how n2 gets its guard above: n3 maps over n0, and n2 appears only in
//!    the map body.
//!
//! 2. A node whose consumers all carry guard `non_empty(X)` inherits that
//!    guard. Whenever X is empty, every consumer is skipped, so the node's
//!    output goes unread too. This is how n1 gets its guard: its only
//!    consumer, n2, is guarded on n0.
//!
//! # When a node gets no guard
//!
//! The analysis is conservative: one consumer that might read the output
//! kills the guard, and the node always executes. In particular:
//!
//! - A node that writes to the database (`Operation::is_effectful`) is never
//!   skipped, and never propagates a guard to its inputs.
//! - The completion node's output is the query result; the engine reads it
//!   unconditionally.
//! - Consumers guarded on different nodes disagree, so their shared input
//!   gets no guard.
//! - A guard the planner assigned directly ([`Cond::Expr`]) is kept as-is
//!   and does not propagate: when it fails, the node produces a placeholder
//!   value instead of being unread, so its inputs must still execute.
//!
//! In the example, n0 stays unguarded: n3 reads it as the map base, not from
//! the map body, and n3 itself has no guard.
//!
//! # How the pass runs
//!
//! The pass keeps one [`Consumption`] value per node. It summarizes how the
//! consumers visited so far read that node's output, and is one of three
//! states: no consumer visited yet ([`Consumption::None`]), every consumer
//! so far reads the output only when node X returned rows
//! ([`Consumption::OnlyUnder`]`(X)`), or some consumer reads the output
//! regardless ([`Consumption::Observed`]).
//!
//! The pass visits nodes in reverse execution order. A node always executes
//! before its consumers, so the reverse walk visits every consumer first,
//! and a node's [`Consumption`] is complete by the time the walk reaches
//! it. Visiting a node does two things:
//!
//! 1. Decide the node's guard from its own [`Consumption`]: when every
//!    consumer reads the output only when X returned rows, the node gets
//!    guard `non_empty(X)`.
//!
//! 2. Update the [`Consumption`] of each input the node reads. An input read
//!    only from a map body contributes "read only when the map base returned
//!    rows" (rule 1). Any other input is read whenever the node itself runs,
//!    so it contributes the node's own `non_empty` guard — or "read
//!    regardless" when the node has none (rule 2).
//!
//! In the example, the walk visits n3, n2, n1, n0. n3 reads n2 only from its
//! map body over n0, so visiting n3 sets n2's [`Consumption`] to "read only
//! when n0 returned rows". Visiting n2 turns that into guard
//! `non_empty(n0)`, and — since n2 now carries that guard — sets n1's
//! [`Consumption`] to the same state. Visiting n1 turns it into the same
//! guard.
//!
//! The pass runs once, at `LogicalPlan::new`, after planning has produced
//! the complete graph. Guards cannot be assigned earlier, while statements
//! are still being planned: a guard is only valid if every consumer agrees,
//! and planning may still add a consumer that reads the node
//! unconditionally.

use index_vec::IndexVec;
use indexmap::IndexSet;
use toasty_core::stmt::{self, visit};

use super::{Cond, NodeId, Operation, Store};

/// The merge, over a node's already-visited consumers, of how each one
/// consumes the node's output.
#[derive(Clone, Copy)]
enum Consumption {
    /// No consumer visited yet.
    None,

    /// Every consumer so far is unobservable when the referenced node's
    /// output is empty.
    OnlyUnder(NodeId),

    /// Some consumer observes the output unconditionally, or consumers
    /// disagree on the condition; the node must always execute.
    Observed,
}

pub(super) fn annotate_guards(store: &mut Store, execution_order: &[NodeId], completion: NodeId) {
    // Execution position; a guard's condition node must run before the nodes
    // it guards so the condition variable exists when the branch is taken.
    // Pure nodes unreachable from the completion node are absent from the
    // execution order and keep the zero sentinel — never read, since guard
    // subjects are inputs of reachable nodes and inputs are deps.
    let mut position: IndexVec<NodeId, usize> = IndexVec::from_vec(vec![0; store.node_count()]);
    for (i, &id) in execution_order.iter().enumerate() {
        position[id] = i;
    }

    let mut consumption: IndexVec<NodeId, Consumption> =
        IndexVec::from_vec(vec![Consumption::None; store.node_count()]);

    for &id in execution_order.iter().rev() {
        // Effects always run; the completion node is consumed by the engine.
        // A planner-assigned guard (`Cond::Expr`) is left untouched.
        let eligible =
            !store[id].op.is_effectful() && id != completion && store[id].guard.is_none();
        if eligible
            && let Consumption::OnlyUnder(x) = consumption[id]
            && position[x] < position[id]
        {
            store[id].guard = Some(Cond::NonEmpty(x));
        }

        // What this node's references contribute to its producers: `Some(x)`
        // when the reference is unobservable under `non_empty(x)` failing
        // because the node itself is guarded on `x` (rule 2), `None`
        // otherwise. Planner-assigned `Cond::Expr` guards do not propagate:
        // their else-arm placeholder is an observed value, not proven
        // unobservable, so producers feeding them must still execute.
        let own = match &store[id].guard {
            Some(Cond::NonEmpty(x)) => Some(*x),
            Some(Cond::Expr { .. }) | None => None,
        };

        // Rule 1: for an `Eval` mapping over one input's rows, inputs
        // referenced only inside the map body contribute `non_empty(base)`
        // regardless of the `Eval`'s own guard.
        let body_only = match &store[id].op {
            Operation::Eval(eval) => {
                map_body_only_positions(eval.eval.expr()).and_then(|(base_pos, positions)| {
                    eval.inputs
                        .get_index(base_pos)
                        .map(|&base| (base, positions))
                })
            }
            _ => None,
        };

        // Push onto the value edges — read from operation inputs, never from
        // `deps`, so an ordering-only edge never looks like a consumer.
        for (pos, input) in store[id].op.inputs().into_iter().enumerate() {
            let contribution = match &body_only {
                Some((base, positions)) if positions.contains(&pos) => Some(*base),
                _ => own,
            };
            consumption[input] = match (consumption[input], contribution) {
                (Consumption::None, Some(x)) => Consumption::OnlyUnder(x),
                (Consumption::OnlyUnder(prev), Some(x)) if prev == x => {
                    Consumption::OnlyUnder(prev)
                }
                _ => Consumption::Observed,
            };
        }
    }
}

/// Detects the `Eval` function form that rule 1 applies to, and reports
/// which inputs are read only from the map body.
///
/// An `Eval`'s function is an expression evaluated against the node's
/// inputs. The expression refers to an input by position: `arg(0)` is the
/// first input, `arg(1)` the second, and so on. Rule 1 looks for a function
/// that is exactly one map over an input's rows:
///
/// ```text
/// map(arg(b), body)
/// ```
///
/// The map evaluates `body` once per row of input `b`; zero rows means zero
/// evaluations. And because the map is the whole function, the base
/// `arg(b)` and the body are the only places an input can be read from. So
/// every input the body reads, other than `b` itself, is read only when `b`
/// returns rows.
///
/// One wrinkle: inside the body, `arg` is relative to the map's row scope.
/// `arg(0)` with `nesting == 0` is the current row, and each level of
/// `nesting` steps out one scope. A body arg refers to an `Eval` input when
/// its nesting matches the number of scopes around it — that is what the
/// walk below checks, starting at depth 1 for the body itself.
///
/// For n3 in the module example, with inputs `[n0, n2]`:
///
/// ```text
/// map(arg(0), <build a todo from the current row, taking its user from arg(1)>)
/// ```
///
/// The base is input 0 (n0, the todos); the body reads input 1 (n2, the
/// users). The function returns `(0, {1})`: input 1 is read only from the
/// body. Returns `None` when the function does not have the map form, or
/// when the base is not a plain input reference.
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
