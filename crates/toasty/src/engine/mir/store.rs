use std::ops;

use index_vec::IndexVec;
use toasty_core::stmt;

use super::Node;

/// Storage for MIR operation nodes.
///
/// [`Store`] is an arena that holds all [`Node`]s in the operation graph,
/// indexed by [`NodeId`]. It provides insertion and lookup operations used
/// during planning to build the graph incrementally.
#[derive(Debug)]
pub(crate) struct Store {
    /// All nodes in the graph, indexed by [`NodeId`].
    pub(crate) store: IndexVec<NodeId, Node>,
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

impl Store {
    pub(crate) fn new() -> Store {
        Store {
            store: IndexVec::new(),
        }
    }

    /// Insert a node into the graph
    pub(crate) fn insert(&mut self, node: impl Into<Node>) -> NodeId {
        self.store.push(node.into())
    }

    pub(crate) fn insert_with_deps<I>(&mut self, node: impl Into<Node>, deps: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let mut node = node.into();
        node.deps.extend(deps);
        self.store.push(node)
    }

    pub(crate) fn ty(&self, node_id: NodeId) -> &stmt::Type {
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
