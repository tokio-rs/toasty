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

mod node;
pub(crate) use node::Node;

mod operation;
pub(crate) use operation::Operation;

mod project;
pub(crate) use project::Project;

mod query_pk;
pub(crate) use query_pk::QueryPk;

mod read_modify_write;
pub(crate) use read_modify_write::ReadModifyWrite;

mod update_by_key;
pub(crate) use update_by_key::UpdateByKey;

use std::ops;

use index_vec::IndexVec;
use toasty_core::stmt;

use crate::engine::exec;

#[derive(Debug)]
pub(crate) struct Store {
    /// Nodes in the graph
    pub(crate) store: IndexVec<NodeId, Node>,

    /// Order of execution
    pub(crate) execution_order: Vec<NodeId>,
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

impl Store {
    pub(super) fn new() -> Store {
        Store {
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

impl ops::Index<NodeId> for Store {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::IndexMut<NodeId> for Store {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        self.store.index_mut(index)
    }
}

impl ops::Index<&NodeId> for Store {
    type Output = Node;

    fn index(&self, index: &NodeId) -> &Self::Output {
        self.store.index(*index)
    }
}

impl ops::IndexMut<&NodeId> for Store {
    fn index_mut(&mut self, index: &NodeId) -> &mut Self::Output {
        self.store.index_mut(*index)
    }
}
