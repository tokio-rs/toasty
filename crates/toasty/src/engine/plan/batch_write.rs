use super::*;

#[derive(Debug, Default)]
pub(crate) struct BatchWrite {
    /// Items being batch written.
    pub items: Vec<WriteAction>,
}

#[derive(Debug)]
pub(crate) enum WriteAction {
    DeleteByKey(DeleteByKey),
    Insert(Insert),
    UpdateByKey(UpdateByKey),
}

impl WriteAction {
    pub(crate) fn as_insert(&self) -> &Insert {
        match self {
            Self::Insert(action) => action,
            _ => panic!(),
        }
    }

    pub(crate) fn as_insert_mut(&mut self) -> &mut Insert {
        match self {
            Self::Insert(action) => action,
            _ => panic!(),
        }
    }
}

impl From<WriteAction> for Action {
    fn from(value: WriteAction) -> Self {
        match value {
            WriteAction::DeleteByKey(stage) => Self::DeleteByKey(stage),
            WriteAction::Insert(stage) => Self::Insert(stage),
            WriteAction::UpdateByKey(stage) => Self::UpdateByKey(stage),
        }
    }
}

impl From<BatchWrite> for Action {
    fn from(value: BatchWrite) -> Self {
        Self::BatchWrite(value)
    }
}

impl From<DeleteByKey> for WriteAction {
    fn from(value: DeleteByKey) -> Self {
        Self::DeleteByKey(value)
    }
}

impl From<Insert> for WriteAction {
    fn from(value: Insert) -> Self {
        Self::Insert(value)
    }
}

impl From<UpdateByKey> for WriteAction {
    fn from(value: UpdateByKey) -> Self {
        Self::UpdateByKey(value)
    }
}
