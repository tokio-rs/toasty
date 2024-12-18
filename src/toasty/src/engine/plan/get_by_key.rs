use super::*;
use crate::schema::*;

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// Where to store the result
    pub output: VarId,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// Keys to get
    pub keys: eval::Func,

    /// How to project the columns after receiving them from the database.
    pub project: eval::Func,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub post_filter: Option<eval::Func>,
}

impl From<GetByKey> for Action {
    fn from(src: GetByKey) -> Action {
        Action::GetByKey(src)
    }
}
