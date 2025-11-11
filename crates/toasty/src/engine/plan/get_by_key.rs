use crate::engine::plan::{Action, Output, VarId};
use toasty_core::schema::db::{ColumnId, TableId};

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Where to get the keys to load
    pub input: VarId,

    /// Where to store the result
    pub output: Output,

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
