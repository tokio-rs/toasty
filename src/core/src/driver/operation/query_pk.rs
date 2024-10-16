use super::*;

use crate::schema::{ColumnId, TableId};

#[derive(Debug)]
pub struct QueryPk<'stmt> {
    /// Table to query
    pub table: TableId,

    /// Which columns to get
    pub select: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr<'stmt>,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub filter: Option<stmt::Expr<'stmt>>,
}

impl<'stmt> From<QueryPk<'stmt>> for Operation<'stmt> {
    fn from(value: QueryPk<'stmt>) -> Self {
        Operation::QueryPk(value)
    }
}
