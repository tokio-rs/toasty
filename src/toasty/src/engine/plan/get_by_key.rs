use super::*;
use crate::schema::*;

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Where to get arguments for this action.
    pub input: Vec<Input>,

    /// Where to store the result
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// Keys to get
    pub keys: eval::Expr,

    /// How to project the columns after receiving them from the database.
    pub project: eval::Expr,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub post_filter: Option<eval::Expr>,
}

impl From<GetByKey> for Action {
    fn from(src: GetByKey) -> Action {
        Action::GetByKey(src)
    }
}
