use super::*;

use crate::{
    schema::db::{ColumnId, TableId},
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
}

impl From<GetByKey> for Operation {
    fn from(value: GetByKey) -> Self {
        Self::GetByKey(value)
    }
}
