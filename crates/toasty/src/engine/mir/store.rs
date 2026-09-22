use std::ops;

use index_vec::IndexVec;
use toasty_core::stmt;

use super::Node;

/// Storage for MIR operation nodes.
///
/// [`Store`] is an arena that holds all [`Node`]s in the operation graph,
/// indexed by [`NodeId`]. It provides insertion and lookup operations used
/// during planning to build the graph incrementally.
///
/// A slot may be reserved before its node exists: [`Store::reserve`] hands
/// out a [`NodeId`] that other nodes can reference immediately, and
/// [`Store::fill`] supplies the node later — typically an
/// [`Alias`](super::Alias) pass-through to whatever node planning actually
/// produced. This lets a statement be referenced (e.g. by an ordering edge
/// from a descendant) before it is planned. Every reserved slot must be
/// filled before the graph is used.
#[derive(Debug)]
pub(crate) struct Store {
    /// All slots in the graph, indexed by [`NodeId`].
    slots: IndexVec<NodeId, Slot>,
}

// Boxing `Node` to shrink the (transient, rare) `Reserved` variant would cost
// an allocation and an indirection on every node access.
#[expect(clippy::large_enum_variant)]
#[derive(Debug)]
enum Slot {
    /// Allocated by [`Store::reserve`]; awaiting its node.
    Reserved,

    /// A filled slot holding its node.
    Node(Node),
}

index_vec::define_index_type! {
    pub(crate) struct NodeId = u32;
}

impl Store {
    pub(crate) fn new() -> Store {
        Store {
            slots: IndexVec::new(),
        }
    }

    /// Insert a node into the graph
    pub(crate) fn insert(&mut self, node: impl Into<Node>) -> NodeId {
        self.slots.push(Slot::Node(node.into()))
    }

    pub(crate) fn insert_with_deps<I>(&mut self, node: impl Into<Node>, deps: I) -> NodeId
    where
        I: IntoIterator<Item = NodeId>,
    {
        let mut node = node.into();
        node.deps.extend(deps);
        self.slots.push(Slot::Node(node))
    }

    /// Allocate a slot whose node is supplied later via [`Store::fill`].
    pub(crate) fn reserve(&mut self) -> NodeId {
        self.slots.push(Slot::Reserved)
    }

    /// Fill a reserved slot with its node.
    #[track_caller]
    pub(crate) fn fill(&mut self, id: NodeId, node: impl Into<Node>) {
        let slot = &mut self.slots[id];
        assert!(
            matches!(slot, Slot::Reserved),
            "fill of non-reserved slot {id:?}"
        );
        *slot = Slot::Node(node.into());
    }

    /// True when no reserved slot is left unfilled.
    pub(crate) fn all_filled(&self) -> bool {
        self.slots.iter().all(|slot| matches!(slot, Slot::Node(_)))
    }

    /// Number of slots, filled or reserved. [`NodeId`]s are dense in
    /// `0..node_count()`, so this sizes per-node side tables.
    pub(crate) fn node_count(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn ty(&self, node_id: NodeId) -> &stmt::Type {
        self[node_id].ty()
    }
}

impl ops::Index<NodeId> for Store {
    type Output = Node;

    #[track_caller]
    fn index(&self, index: NodeId) -> &Self::Output {
        match &self.slots[index] {
            Slot::Node(node) => node,
            Slot::Reserved => panic!("reserved MIR slot {index:?} read before fill"),
        }
    }
}

impl ops::IndexMut<NodeId> for Store {
    #[track_caller]
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        match &mut self.slots[index] {
            Slot::Node(node) => node,
            Slot::Reserved => panic!("reserved MIR slot {index:?} read before fill"),
        }
    }
}

impl ops::Index<&NodeId> for Store {
    type Output = Node;

    #[track_caller]
    fn index(&self, index: &NodeId) -> &Self::Output {
        self.index(*index)
    }
}

impl ops::IndexMut<&NodeId> for Store {
    #[track_caller]
    fn index_mut(&mut self, index: &NodeId) -> &mut Self::Output {
        self.index_mut(*index)
    }
}
