use super::{eval, stmt, Action, Output, VarId};
use toasty_core::schema::db::{ColumnId, TableId};

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

#[derive(Debug)]
pub(crate) struct QueryPk2 {
    /// Where to store the result
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Filter to pass to the database
    pub row_filter: Option<stmt::Expr>,
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}

impl From<QueryPk2> for Action {
    fn from(value: QueryPk2) -> Self {
        Action::QueryPk2(value)
    }
}
