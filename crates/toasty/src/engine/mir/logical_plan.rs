use std::ops;

use crate::engine::mir::{Node, NodeId, Store, annotate_guards};

/// The complete operation graph for a query.
///
/// [`LogicalPlan`] is a directed acyclic graph of operations produced by the
/// planning phase. It contains all nodes, their topologically sorted execution
/// order, and the final completion node whose output is returned to the user.
#[derive(Debug)]
pub(crate) struct LogicalPlan {
    /// All nodes in the operation graph.
    store: Store,

    /// Topologically sorted order in which to execute operations.
    execution_order: Vec<NodeId>,

    /// The final node whose output is the query result.
    completion: NodeId,
}

impl LogicalPlan {
    pub(crate) fn new(mut store: Store, completion: NodeId) -> LogicalPlan {
        let mut execution_order = vec![];
        compute_operation_execution_order(completion, &store, &mut execution_order);

        // Every reserved slot must be filled by the time planning completes —
        // an unfilled slot means a statement was referenced but never planned.
        debug_assert!(
            store.all_filled(),
            "reserved MIR slot left unfilled at plan completion"
        );

        // Nodes unreachable from the completion node are dropped from the
        // execution order and never run. That is fine for pure nodes; a
        // dropped mutation would silently lose its database effect.
        debug_assert!(
            store
                .nodes()
                .all(|node| node.visited.get() || !node.op.is_effectful()),
            "effectful node unreachable from the completion node"
        );

        // `num_uses` counts the variable loads each node's output receives:
        // one per entry in its consumers' `input_loads()`. Ordering-only
        // `deps` edges schedule but do not count — no load ever drains them.
        // The completion node's exit use is added by the caller before this
        // point.
        for node_id in &execution_order {
            for load in store[node_id].op.input_loads() {
                let dep = &store[load];
                dep.num_uses.set(dep.num_uses.get() + 1);
            }
        }

        annotate_guards(&mut store, &execution_order, completion);

        LogicalPlan {
            store,
            execution_order,
            completion,
        }
    }

    /// Node IDs in topologically sorted execution order.
    pub(crate) fn execution_order(&self) -> &[NodeId] {
        &self.execution_order
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
        compute_operation_execution_order(dep_id, mir, execution_order);
    }

    execution_order.push(node_id);
}
