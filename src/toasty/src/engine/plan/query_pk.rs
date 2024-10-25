use super::*;
use crate::{driver::*, schema::*};

#[derive(Debug)]
pub(crate) struct QueryPk<'stmt> {
    /// Where to store the result
    pub output: plan::VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr<'stmt>,

    /// How to project the result returned by the driver
    pub project: eval::Expr<'stmt>,

    /// Filter to pass to the database
    pub filter: Option<stmt::Expr<'stmt>>,

    /// Filter to apply in-memory
    pub post_filter: Option<eval::Expr<'stmt>>,
}

impl<'stmt> QueryPk<'stmt> {
    pub(crate) fn apply(&self) -> Result<operation::QueryPk<'stmt>> {
        Ok(operation::QueryPk {
            table: self.table,
            select: self.columns.clone(),
            pk_filter: self.pk_filter.clone(),
            filter: self.filter.clone(),
        })
    }
}

impl<'stmt> From<QueryPk<'stmt>> for Action<'stmt> {
    fn from(value: QueryPk<'stmt>) -> Self {
        Action::QueryPk(value)
    }
}
