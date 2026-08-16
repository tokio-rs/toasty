use indexmap::IndexSet;
use toasty_core::stmt;

use super::{NodeId, Operation};

/// A single node in the MIR operation graph.
///
/// Each [`Node`] represents one operation to execute. It contains the
/// operation itself, its dependencies on other nodes, and its execution
/// guard.
#[derive(Debug)]
pub(crate) struct Node {
    /// The operation this node performs.
    pub(crate) op: Operation,

    /// Nodes that must execute before this one.
    ///
    /// This is a superset of the node's data inputs; it may include additional
    /// ordering dependencies (e.g., an `UPDATE` depending on a prior `INSERT`).
    pub(crate) deps: IndexSet<NodeId>,

    /// When set, this node only executes if the referenced node produced at
    /// least one row, evaluated with a non-consuming peek. Assigned by the
    /// guard-annotation pass in [`LogicalPlan::new`], which proves the
    /// guarded node's output unobservable when the condition fails; only
    /// pure (non-effectful) nodes may carry a guard.
    ///
    /// [`LogicalPlan::new`]: super::LogicalPlan::new
    pub(crate) guard: Option<NodeId>,
}

impl Node {
    pub(crate) fn ty(&self) -> &stmt::Type {
        match &self.op {
            Operation::Alias(m) => &m.ty,
            Operation::Const(m) => &m.ty,
            Operation::DeleteByKey(m) => &m.ty,
            Operation::Eval(m) => &m.ty,
            Operation::ExecStatement(m) => &m.ty,
            Operation::Filter(m) => &m.ty,
            Operation::FindPkByIndex(m) => &m.ty,
            Operation::GetByKey(m) => &m.ty,
            Operation::QueryPk(m) => &m.ty,
            Operation::Repeat(m) => &m.ty,
            Operation::Scan(m) => &m.ty,
            Operation::UpdateByKey(m) => &m.ty,
            Operation::Upsert(m) => &m.ty,
            Operation::NestedMerge(m) => &m.ty,
            Operation::ReadModifyWrite(m) => &m.ty,
        }
    }
}
