use crate::engine::plan::{Action, Output, VarId};
use toasty_core::{
    schema::db::{ColumnId, TableId},
    stmt,
};

#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Where to get the input
    pub input: Option<VarId>,

    /// Where to store the result
    pub output: Output,

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
        Action::QueryPk(value)
    }
}
