use super::*;

use crate::schema::{IndexId, TableId};

#[derive(Debug)]
pub struct FindPkByIndex<'stmt> {
    /// Table to query
    pub table: TableId,

    /// Which index to query
    pub index: IndexId,

    /// How to filter the index.
    pub filter: sql::Expr<'stmt>,
}

impl<'stmt> From<FindPkByIndex<'stmt>> for Operation<'stmt> {
    fn from(value: FindPkByIndex<'stmt>) -> Operation<'stmt> {
        Operation::FindPkByIndex(value)
    }
}
