use super::*;

#[derive(Debug, Default)]
pub(crate) struct BatchWrite<'stmt> {
    /// Items being batch written.
    pub items: Vec<WriteAction<'stmt>>,
}

#[derive(Debug)]
pub(crate) enum WriteAction<'stmt> {
    DeleteByKey(DeleteByKey<'stmt>),
    Insert(Insert<'stmt>),
    UpdateByKey(UpdateByKey<'stmt>),
}

impl<'stmt> WriteAction<'stmt> {
    pub(crate) fn as_insert_mut(&mut self) -> &mut Insert<'stmt> {
        match self {
            WriteAction::Insert(action) => action,
            _ => panic!(),
        }
    }
}

impl<'stmt> From<WriteAction<'stmt>> for Action<'stmt> {
    fn from(value: WriteAction<'stmt>) -> Self {
        match value {
            WriteAction::DeleteByKey(stage) => Action::DeleteByKey(stage),
            WriteAction::Insert(stage) => Action::Insert(stage),
            WriteAction::UpdateByKey(stage) => Action::UpdateByKey(stage),
        }
    }
}

impl<'stmt> From<BatchWrite<'stmt>> for Action<'stmt> {
    fn from(value: BatchWrite<'stmt>) -> Self {
        Action::BatchWrite(value)
    }
}

impl<'stmt> From<DeleteByKey<'stmt>> for WriteAction<'stmt> {
    fn from(value: DeleteByKey<'stmt>) -> Self {
        WriteAction::DeleteByKey(value)
    }
}

impl<'stmt> From<Insert<'stmt>> for WriteAction<'stmt> {
    fn from(value: Insert<'stmt>) -> Self {
        WriteAction::Insert(value)
    }
}

impl<'stmt> From<UpdateByKey<'stmt>> for WriteAction<'stmt> {
    fn from(value: UpdateByKey<'stmt>) -> Self {
        WriteAction::UpdateByKey(value)
    }
}
