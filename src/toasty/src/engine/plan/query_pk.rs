use super::*;
use crate::{driver::*, schema::*};

#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Where to store the result
    pub output: plan::VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr<'static>,

    /// How to project the result returned by the driver
    pub project: eval::Expr,

    /// Filter to pass to the database
    pub filter: Option<stmt::Expr<'static>>,

    /// Filter to apply in-memory
    pub post_filter: Option<eval::Expr>,
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
