use crate::engine::exec::{Action, VarId, VarStore};

#[derive(Debug)]
pub(crate) struct ExecPlan {
    pub(crate) vars: VarStore,
    pub(crate) blocks: Vec<Block>,
    pub(crate) entry: BlockId,
    pub(crate) returning: Option<VarId>,
    pub(crate) needs_transaction: bool,
}

pub(crate) type BlockId = usize;

#[derive(Debug)]
pub(crate) struct Block {
    pub(crate) actions: Vec<Action>,
    pub(crate) terminator: Terminator,
}

#[derive(Debug)]
pub(crate) enum Terminator {
    Goto(BlockId),
    IfNonEmpty {
        var: VarId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return,
}
