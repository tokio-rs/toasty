use index_vec::IndexVec;

use crate::engine::exec::{Block, BlockId, VarId, VarStore};

#[derive(Debug)]
pub(crate) struct ExecPlan {
    /// Arguments seeding the plan
    pub(crate) vars: VarStore,

    /// Basic blocks forming the control flow graph.
    /// The entry block is always index 0.
    pub(crate) blocks: IndexVec<BlockId, Block>,

    /// Which record stream slot does the pipeline return
    ///
    /// When `None`, nothing is returned
    pub(crate) returning: Option<VarId>,

    /// When true, the executor wraps the entire plan in a transaction.
    pub(crate) needs_transaction: bool,
}
