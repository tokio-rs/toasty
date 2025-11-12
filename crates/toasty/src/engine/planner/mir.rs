mod r#const;
pub(crate) use r#const::Const;

mod delete_by_key;
pub(crate) use delete_by_key::DeleteByKey;

mod exec_statement;
pub(crate) use exec_statement::ExecStatement;

mod filter;
pub(crate) use filter::Filter;

mod find_pk_by_index;
pub(crate) use find_pk_by_index::FindPkByIndex;

mod get_by_key;
pub(crate) use get_by_key::GetByKey;

mod nested_merge;
pub(crate) use nested_merge::NestedMerge;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod read_modify_write;
pub(crate) use read_modify_write::ReadModifyWrite;

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use std::{cell::Cell, ops};

use index_vec::IndexVec;
use indexmap::{indexset, IndexSet};
use toasty_core::stmt;

use crate::engine::exec;

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

/// Materialization operation
#[derive(Debug)]
pub(crate) enum Operation {
    /// A constant value
    Const(Const),

    DeleteByKey(DeleteByKey),

    /// Execute a database query
    ExecStatement(Box<ExecStatement>),

    /// Filter results
    Filter(Filter),

    /// Find primary keys by index
    FindPkByIndex(FindPkByIndex),

    /// Get records by primary key
    GetByKey(GetByKey),

    /// Execute a nested merge
    NestedMerge(NestedMerge),

    /// Projection operation - transforms records
    Project(Project),

    /// Read-modify-write. The write only succeeds if the values read are not
    /// modified.
    ReadModifyWrite(Box<ReadModifyWrite>),

    QueryPk(QueryPk),

    UpdateByKey(UpdateByKey),
}

#[derive(Debug)]
pub(crate) struct MaterializeGraph {
    /// Nodes in the graph
    pub(crate) store: IndexVec<NodeId, Node>,

    /// Order of execution
    pub(crate) execution_order: Vec<NodeId>,
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

impl MaterializeGraph {
    pub(super) fn new() -> MaterializeGraph {
        MaterializeGraph {
            store: IndexVec::new(),
            execution_order: vec![],
        }
    }

    /// Insert a node into the graph
    pub(super) fn insert(&mut self, node: impl Into<Node>) -> NodeId {
        self.store.push(node.into())
    }

    pub(super) fn insert_with_deps<I>(&mut self, node: impl Into<Node>, deps: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let mut node = node.into();
        node.deps.extend(deps);
        self.store.push(node)
    }

    pub(super) fn var_id(&self, node_id: NodeId) -> exec::VarId {
        self.store[node_id].var_id()
    }

    pub(super) fn ty(&self, node_id: NodeId) -> &stmt::Type {
        self.store[node_id].ty()
    }
}

impl Node {
    pub(super) fn ty(&self) -> &stmt::Type {
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
}

impl ops::Index<NodeId> for MaterializeGraph {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::IndexMut<NodeId> for MaterializeGraph {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        self.store.index_mut(index)
    }
}

impl ops::Index<&NodeId> for MaterializeGraph {
    type Output = Node;

    fn index(&self, index: &NodeId) -> &Self::Output {
        self.store.index(*index)
    }
}

impl ops::IndexMut<&NodeId> for MaterializeGraph {
    fn index_mut(&mut self, index: &NodeId) -> &mut Self::Output {
        self.store.index_mut(*index)
    }
}

impl From<Operation> for Node {
    fn from(value: Operation) -> Self {
        let deps = match &value {
            Operation::Const(_m) => IndexSet::new(),
            Operation::DeleteByKey(m) => indexset![m.input],
            Operation::ExecStatement(m) => m.inputs.clone(),
            Operation::Filter(m) => indexset![m.input],
            Operation::FindPkByIndex(m) => m.inputs.clone(),
            Operation::GetByKey(m) => {
                indexset![m.input]
            }
            Operation::NestedMerge(m) => m.inputs.clone(),
            Operation::Project(m) => indexset![m.input],
            Operation::ReadModifyWrite(m) => m.inputs.clone(),
            Operation::QueryPk(m) => m.input.into_iter().collect(),
            Operation::UpdateByKey(m) => indexset![m.input],
        };

        Node {
            op: value,
            deps,
            var: Cell::new(None),
            num_uses: Cell::new(0),
            visited: Cell::new(false),
        }
    }
}
