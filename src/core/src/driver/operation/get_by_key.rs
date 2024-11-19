use super::*;

use crate::{
    schema::{ColumnId, TableId},
    stmt,
};

#[derive(Debug)]
pub struct GetByKey<'stmt> {
    /// Which table to get from
    pub table: TableId,

    /// Which columns to select
    pub select: Vec<ColumnId>,

    /// Which keys to fetch
    pub keys: Vec<stmt::Value<'stmt>>,

    /// How to filter the result before returning it to the caller.
    /// TODO: this needs to be moved to the engine
    pub post_filter: Option<eval::Expr>,
}

impl<'stmt> From<GetByKey<'stmt>> for Operation<'stmt> {
    fn from(value: GetByKey<'stmt>) -> Operation<'stmt> {
        Operation::GetByKey(value)
    }
}
