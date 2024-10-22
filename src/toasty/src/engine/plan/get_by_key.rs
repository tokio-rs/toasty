use super::*;
use crate::schema::*;

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey<'stmt> {
    /// Where to get arguments for this action.
    pub input: Vec<Input<'stmt>>,

    /// Where to store the result
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// Keys to get
    pub keys: eval::Expr<'stmt>,

    /// How to project the columns after receiving them from the database.
    pub project: eval::Expr<'stmt>,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub post_filter: Option<eval::Expr<'stmt>>,
}

impl<'stmt> From<GetByKey<'stmt>> for Action<'stmt> {
    fn from(src: GetByKey<'stmt>) -> Action<'stmt> {
        Action::GetByKey(src)
    }
}
