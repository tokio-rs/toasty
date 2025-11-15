use std::ops;

use crate::engine::mir::{Node, NodeId, Store};

#[derive(Debug)]
pub(crate) struct LogicalPlan {
    /// Nodes in the operation graph
    store: Store,

    /// Order in which to execute the operations
    execution_order: Vec<NodeId>,

    /// Final node representing the completion of the query
    completion: NodeId,
}

impl LogicalPlan {
    pub(crate) fn new(
        store: Store,
        execution_order: Vec<NodeId>,
        completion: NodeId,
    ) -> LogicalPlan {
        LogicalPlan {
            store,
            execution_order,
            completion,
        }
    }

    pub(crate) fn operations(&self) -> impl Iterator<Item = &Node> {
        self.execution_order
            .iter()
            .map(|node_id| &self.store[node_id])
    }

    pub(crate) fn completion(&self) -> &Node {
        &self.store[self.completion]
    }
}

impl ops::Index<NodeId> for LogicalPlan {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::Index<&NodeId> for LogicalPlan {
    type Output = Node;

    fn index(&self, index: &NodeId) -> &Self::Output {
        self.store.index(index)
    }
}
