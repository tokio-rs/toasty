use super::*;

use crate::schema::db::{ColumnId, TableId};

#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Where to store the result
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Filter to pass to the database
    pub filter: Option<stmt::Expr>,

    /// Filter to apply in-memory
    pub post_filter: Option<eval::Func>,
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
