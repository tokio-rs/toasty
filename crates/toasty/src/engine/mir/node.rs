use std::cell::Cell;

use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{exec, planner::VarTable};

use super::{NodeId, Operation, Store};

#[derive(Debug)]
pub(crate) struct Node {
    /// Materialization kind
    pub(crate) op: Operation,

    /// Nodes that must execute *before* the current one. This should be a
    /// superset of the node's inputs.
    pub(crate) deps: IndexSet<NodeId>,

    /// Variable where the output is stored
    pub(crate) var: Cell<Option<exec::VarId>>,

    /// Number of nodes that use this one as input.
    pub(crate) num_uses: Cell<usize>,

    /// Used for topo-sort
    pub(crate) visited: Cell<bool>,
}

impl Node {
    pub(crate) fn ty(&self) -> &stmt::Type {
        match &self.op {
            Operation::Const(m) => &m.ty,
            Operation::DeleteByKey(m) => &m.ty,
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

    pub(super) fn var_id(&self) -> exec::VarId {
        self.var.get().unwrap()
    }

    pub(crate) fn to_exec(&self, graph: &Store, var_table: &mut VarTable) -> exec::Action {
        match &self.op {
            Operation::Const(op) => op.to_exec(self, var_table).into(),
            Operation::DeleteByKey(op) => op.to_exec(graph, self, var_table).into(),
            Operation::ExecStatement(op) => op.to_exec(graph, self, var_table).into(),
            Operation::Filter(op) => op.to_exec(graph, self, var_table).into(),
            Operation::FindPkByIndex(op) => op.to_exec(graph, self, var_table).into(),
            Operation::GetByKey(op) => op.to_exec(graph, self, var_table).into(),
            Operation::NestedMerge(op) => op.to_exec(graph, self, var_table).into(),
            Operation::Project(op) => op.to_exec(graph, self, var_table).into(),
            Operation::ReadModifyWrite(op) => op.to_exec(graph, self, var_table).into(),
            Operation::QueryPk(op) => op.to_exec(graph, self, var_table).into(),
            Operation::UpdateByKey(op) => op.to_exec(graph, self, var_table).into(),
        }
    }
}
