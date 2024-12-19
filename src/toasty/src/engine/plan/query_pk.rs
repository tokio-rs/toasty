use super::*;
use crate::{driver::*, schema::*};

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

impl QueryPk {
    pub(crate) fn apply(&self) -> Result<operation::QueryPk> {
        Ok(operation::QueryPk {
            table: self.table,
            select: self.columns.clone(),
            pk_filter: self.pk_filter.clone(),
            filter: self.filter.clone(),
        })
    }
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Action::QueryPk(value)
    }
}
