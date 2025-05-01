use super::*;
use crate::{schema::db::TableId, stmt};

#[derive(Debug)]
pub struct DeleteByKey {
    /// Which table to delete from
    pub table: TableId,

    /// Which keys to delete
    pub keys: Vec<stmt::Value>,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,
}

impl From<DeleteByKey> for Operation {
    fn from(value: DeleteByKey) -> Self {
        Self::DeleteByKey(value)
    }
}
