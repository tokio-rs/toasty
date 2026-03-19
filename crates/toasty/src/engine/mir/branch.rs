use toasty_core::stmt;

use super::{NodeId, Operation};

/// A conditional branch in the MIR graph.
///
/// Evaluates the condition node (which must produce a boolean), then
/// executes either the then_body or else_body nodes. The output of
/// the chosen branch is the Branch node's result.
///
/// Body nodes are NOT included in the main topological execution order.
/// They are marked as visited before the sort runs, and are processed
/// by plan_execution when it encounters the Branch.
#[derive(Debug)]
pub(crate) struct Branch {
    /// Node that produces a boolean condition value.
    pub(crate) cond: NodeId,

    /// Nodes to execute when the condition is true. The last node's
    /// output is the then-branch result.
    pub(crate) then_body: Vec<NodeId>,

    /// The type of the Branch output.
    pub(crate) ty: stmt::Type,
}

impl From<Branch> for super::Node {
    fn from(value: Branch) -> Self {
        Operation::Branch(value).into()
    }
}
