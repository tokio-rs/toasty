use super::*;

use crate::{
    schema::{ColumnId, TableId},
    stmt,
};

#[derive(Debug)]
pub struct GetByKey {
    /// Which table to get from
    pub table: TableId,

    /// Which columns to select
    pub select: Vec<ColumnId>,

    /// Which keys to fetch
    pub keys: Vec<stmt::Value>,

    /// How to filter the result before returning it to the caller.
    /// TODO: this needs to be moved to the engine
    pub post_filter: Option<eval::Expr>,
}

impl From<GetByKey> for Operation {
    fn from(value: GetByKey) -> Operation {
        Operation::GetByKey(value)
    }
}
