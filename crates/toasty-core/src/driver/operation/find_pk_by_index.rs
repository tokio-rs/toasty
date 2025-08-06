use super::Operation;
use crate::{schema::db::{IndexId, TableId}, stmt};

#[derive(Debug)]
pub struct FindPkByIndex {
    /// Table to query
    pub table: TableId,

    /// Which index to query
    pub index: IndexId,

    /// How to filter the index.
    pub filter: stmt::Expr,
}

impl From<FindPkByIndex> for Operation {
    fn from(value: FindPkByIndex) -> Self {
        Self::FindPkByIndex(value)
    }
}
