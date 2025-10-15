use crate::engine::plan::{Output2, VarId};

use super::{eval, Action, Input, Output};

use toasty_core::schema::db::{ColumnId, TableId};

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Where to get arguments for this action.
    pub input: Option<Input>,

    /// Where to store the result
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Keys to get
    pub keys: eval::Func,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// Additional filtering done on the result before returning it to the
    /// caller.
    pub post_filter: Option<eval::Func>,
}

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey2 {
    /// Where to get the keys to load
    pub input: VarId,

    /// Where to store the result
    pub output: Output2,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,
}

impl From<GetByKey> for Action {
    fn from(src: GetByKey) -> Self {
        Self::GetByKey(src)
    }
}

impl From<GetByKey2> for Action {
    fn from(src: GetByKey2) -> Self {
        Self::GetByKey2(src)
    }
}
