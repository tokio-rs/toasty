use crate::engine::mir::{NodeId, Store};

#[derive(Debug)]
pub(crate) struct LogicalPlan {
    /// Nodes in the operation graph
    store: Store,

    /// Order in which to execute the operations
    execution_order: Vec<NodeId>,
}

impl LogicalPlan {
    pub(crate) fn new(store: Store, execution_order: Vec<NodeId>) -> LogicalPlan {
        LogicalPlan {
            store,
            execution_order,
        }
    }
}
