use super::*;

use crate::schema::{ColumnId, TableId};

#[derive(Debug)]
pub struct QueryPk {
    /// Table to query
    pub table: TableId,

    /// Which columns to get
    pub select: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr<'static>,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub filter: Option<stmt::Expr<'static>>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Operation::QueryPk(value)
    }
}
