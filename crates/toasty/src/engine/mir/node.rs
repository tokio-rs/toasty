use std::cell::Cell;

use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::exec;

use super::{LogicalPlan, NodeId, Operation};

/// A single node in the MIR operation graph.
///
/// Each [`Node`] represents one operation to execute. It contains the operation
/// itself, its dependencies on other nodes, and metadata used during execution
/// planning (variable assignment, reference counting, traversal state).
#[derive(Debug)]
pub(crate) struct Node {
    /// The operation this node performs.
    pub(crate) op: Operation,

    /// Nodes that must execute before this one.
    ///
    /// This is a superset of the node's data inputs; it may include additional
    /// ordering dependencies (e.g., an `UPDATE` depending on a prior `INSERT`).
    pub(crate) deps: IndexSet<NodeId>,

    /// Variable slot where this node's output is stored during execution.
    ///
    /// Set during execution planning when converting MIR to actions.
    pub(crate) var: Cell<Option<exec::VarId>>,

    /// Number of downstream nodes that consume this node's output.
    ///
    /// Used for reference counting; the output is freed after the last use.
    pub(crate) num_uses: Cell<usize>,

    /// Whether this node has been visited during topological sort.
    pub(crate) visited: Cell<bool>,
}

impl Node {
    pub(crate) fn ty(&self) -> &stmt::Type {
        match &self.op {
            Operation::Const(m) => &m.ty,
            Operation::DeleteByKey(m) => &m.ty,
            Operation::Eval(m) => &m.eval.ret,
            Operation::ExecStatement(m) => &m.ty,
            Operation::Filter(m) => &m.ty,
            Operation::FindPkByIndex(m) => &m.ty,
            Operation::GetByKey(m) => &m.ty,
            Operation::QueryPk(m) => &m.ty,
            Operation::Project(m) => &m.ty,
            Operation::UpdateByKey(m) => &m.ty,
            Operation::NestedMerge(_m) => todo!(),
            Operation::ReadModifyWrite(m) => &m.ty,
        }
    }

    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        var_table: &mut exec::VarDecls,
    ) -> exec::Action {
        match &self.op {
            Operation::Const(op) => op.to_exec(self, var_table).into(),
            Operation::DeleteByKey(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::Eval(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::ExecStatement(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::Filter(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::FindPkByIndex(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::GetByKey(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::NestedMerge(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::Project(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::ReadModifyWrite(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::QueryPk(op) => op.to_exec(logical_plan, self, var_table).into(),
            Operation::UpdateByKey(op) => op.to_exec(logical_plan, self, var_table).into(),
        }
    }
}
