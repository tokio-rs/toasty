use super::*;
use crate::{schema::TableId, stmt};

#[derive(Debug)]
pub struct DeleteByKey<'stmt> {
    /// Which table to delete from
    pub table: TableId,

    /// Which keys to delete
    pub keys: Vec<stmt::Value<'stmt>>,

    /// Only delete keys that match the filter
    pub filter: Option<sql::Expr<'stmt>>,
}

impl<'stmt> From<DeleteByKey<'stmt>> for Operation<'stmt> {
    fn from(value: DeleteByKey<'stmt>) -> Operation<'stmt> {
        Operation::DeleteByKey(value)
    }
}
