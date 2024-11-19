use super::*;
use crate::{schema::TableId, stmt};

#[derive(Debug)]
pub struct DeleteByKey {
    /// Which table to delete from
    pub table: TableId,

    /// Which keys to delete
    pub keys: Vec<stmt::Value<'static>>,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr<'static>>,
}

impl From<DeleteByKey> for Operation {
    fn from(value: DeleteByKey) -> Operation {
        Operation::DeleteByKey(value)
    }
}
