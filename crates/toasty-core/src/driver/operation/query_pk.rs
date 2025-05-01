use super::*;

use crate::schema::db::{ColumnId, TableId};

#[derive(Debug)]
pub struct QueryPk {
    /// Table to query
    pub table: TableId,

    /// Which columns to get
    pub select: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub filter: Option<stmt::Expr>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
