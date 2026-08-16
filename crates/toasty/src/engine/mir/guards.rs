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
//! (`non_empty(n0)`). At execution time, when n0 comes back empty, n1 and n2
//! are skipped.
//!
//! # The two rules
//!
//! A node may be skipped only when nothing reads its output. Two rules prove
//! that:
//!
//! 1. A node read [`InputRead::PerRowOf`]`(X)` — only while iterating node
//!    X's rows — gets guard `non_empty(X)`. Iterating zero rows never reads
//!    the input, so the node's output goes unread whenever X is empty. This
//!    is how n2 gets its guard above: n3 is a per-row `Eval` over n0, and n2
//!    is one of its attached inputs.
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
//!
//! In the example, n0 stays unguarded: n3 reads it as the map base
//! ([`InputRead::Always`]), and n3 itself has no guard.
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
//! 2. Update the [`Consumption`] of each input the node reads, from the
//!    operation's declared [`InputRead`]s. A [`PerRowOf`](InputRead::PerRowOf)
//!    input contributes "read only when the iterated node returned rows"
//!    (rule 1). An [`Always`](InputRead::Always) input is read whenever the
//!    node itself runs, so it contributes the node's own `non_empty` guard —
//!    or "read regardless" when the node has none (rule 2).
//!
//! In the example, the walk visits n3, n2, n1, n0. n3 declares n2 as
//! `PerRowOf(n0)`, so visiting n3 sets n2's [`Consumption`] to "read only
//! when n0 returned rows". Visiting n2 turns that into guard `non_empty(n0)`,
//! and — since n2 now carries that guard — sets n1's [`Consumption`] to the
//! same state. Visiting n1 turns it into the same guard.
//!
//! The pass runs once, at `LogicalPlan::new`, after planning has produced
//! the complete graph. Guards cannot be assigned earlier, while statements
//! are still being planned: a guard is only valid if every consumer agrees,
//! and planning may still add a consumer that reads the node
//! unconditionally.

use index_vec::IndexVec;

use super::{NodeId, Store, operation::InputRead};

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
        let eligible = !store[id].op.is_effectful() && id != completion;
        if eligible
            && let Consumption::OnlyUnder(x) = consumption[id]
            && position[x] < position[id]
        {
            store[id].guard = Some(x);
        }

        // What this node's `Always` reads contribute to its producers:
        // `Some(x)` when the read is unobservable under `non_empty(x)`
        // failing because the node itself is guarded on `x` (rule 2), `None`
        // otherwise.
        let own = store[id].guard;

        // Push onto the value edges — read from operation inputs, never from
        // `deps`, so an ordering-only edge never looks like a consumer.
        for (input, read) in store[id].op.input_reads() {
            let contribution = match read {
                // Rule 1: read only while iterating `base`'s rows, so
                // unobservable whenever `base` is empty — regardless of the
                // node's own guard.
                InputRead::PerRowOf(base) => Some(base),
                InputRead::Always => own,
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
