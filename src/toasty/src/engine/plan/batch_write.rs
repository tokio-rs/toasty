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
            WriteAction::Insert(action) => action,
            _ => panic!(),
        }
    }

    pub(crate) fn as_insert_mut(&mut self) -> &mut Insert {
        match self {
            WriteAction::Insert(action) => action,
            _ => panic!(),
        }
    }
}

impl From<WriteAction> for Action {
    fn from(value: WriteAction) -> Self {
        match value {
            WriteAction::DeleteByKey(stage) => Action::DeleteByKey(stage),
            WriteAction::Insert(stage) => Action::Insert(stage),
            WriteAction::UpdateByKey(stage) => Action::UpdateByKey(stage),
        }
    }
}

impl From<BatchWrite> for Action {
    fn from(value: BatchWrite) -> Self {
        Action::BatchWrite(value)
    }
}

impl From<DeleteByKey> for WriteAction {
    fn from(value: DeleteByKey) -> Self {
        WriteAction::DeleteByKey(value)
    }
}

impl From<Insert> for WriteAction {
    fn from(value: Insert) -> Self {
        WriteAction::Insert(value)
    }
}

impl From<UpdateByKey> for WriteAction {
    fn from(value: UpdateByKey) -> Self {
        WriteAction::UpdateByKey(value)
    }
}
