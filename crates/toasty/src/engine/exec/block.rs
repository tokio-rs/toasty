use index_vec::IndexVec;

use super::{Action, VarId};

index_vec::define_index_type! {
    pub(crate) struct BlockId = u32;
}

#[derive(Debug)]
pub(crate) struct Block {
    pub(crate) actions: Vec<Action>,
    pub(crate) terminator: Terminator,
}

#[derive(Debug)]
pub(crate) enum Terminator {
    /// End execution.
    Return,
    /// Unconditional jump.
    Goto(BlockId),
    /// Conditional branch based on a boolean VarId.
    If {
        cond: VarId,
        then_block: BlockId,
        else_block: BlockId,
    },
}

/// Helper to build blocks during execution planning.
#[derive(Debug)]
pub(crate) struct BlockBuilder {
    pub(crate) blocks: IndexVec<BlockId, Block>,
}

impl BlockBuilder {
    pub(crate) fn new() -> Self {
        BlockBuilder {
            blocks: IndexVec::new(),
        }
    }

    /// Start a new block and return its ID.
    pub(crate) fn new_block(&mut self) -> BlockId {
        self.blocks.push(Block {
            actions: vec![],
            terminator: Terminator::Return,
        })
    }

    /// Push an action into the given block.
    pub(crate) fn push_action(&mut self, block: BlockId, action: Action) {
        self.blocks[block].actions.push(action);
    }

    /// Set the terminator for the given block.
    pub(crate) fn set_terminator(&mut self, block: BlockId, terminator: Terminator) {
        self.blocks[block].terminator = terminator;
    }
}
