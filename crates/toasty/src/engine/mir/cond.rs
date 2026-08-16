use indexmap::IndexSet;

use crate::engine::eval;

use super::NodeId;

/// A condition gating a [`Node`](super::Node)'s execution.
#[derive(Debug)]
pub(crate) enum Cond {
    /// The referenced node produced at least one row.
    ///
    /// Assigned by the guard-annotation pass, which proves the guarded
    /// node's output unobservable when the condition fails. Evaluated with a
    /// non-consuming peek.
    NonEmpty(NodeId),

    /// A boolean expression over the listed nodes' outputs.
    ///
    /// Assigned by the planner (a KV update's args-only pre-filter): when
    /// false, the guarded node's else-arm placeholder — an empty key list —
    /// is the value its consumer observes, so the downstream mutation
    /// no-ops. Evaluation loads each input once, on both arms.
    Expr {
        /// Boolean expression evaluated against `inputs`.
        func: eval::Func,

        /// Nodes whose outputs are loaded for evaluation.
        inputs: IndexSet<NodeId>,
    },
}

impl Cond {
    /// Whether two guards may share one `If` block.
    ///
    /// `NonEmpty` guards on the same node group; `Expr` guards never group
    /// (each carries its own function, and per-block cond evaluation keeps
    /// the input load counting exact).
    pub(crate) fn groups_with(&self, other: &Cond) -> bool {
        match (self, other) {
            (Cond::NonEmpty(a), Cond::NonEmpty(b)) => a == b,
            _ => false,
        }
    }
}
