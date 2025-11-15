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
    pub(crate) fn new(store: Store, completion: NodeId) -> LogicalPlan {
        let mut execution_order = vec![];
        compute_operation_execution_order(completion, &store, &mut execution_order);

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

fn compute_operation_execution_order(
    node_id: NodeId,
    mir: &Store,
    execution_order: &mut Vec<NodeId>,
) {
    let node = &mir[node_id];

    if node.visited.get() {
        return;
    }

    node.visited.set(true);

    for &dep_id in &node.deps {
        let dep = &mir[dep_id];
        dep.num_uses.set(dep.num_uses.get() + 1);

        compute_operation_execution_order(dep_id, mir, execution_order);
    }

    execution_order.push(node_id);
}
