use crate::engine::plan::{Action, Output2, VarId};
use toasty_core::schema::db::{ColumnId, TableId};

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

impl From<GetByKey2> for Action {
    fn from(src: GetByKey2) -> Self {
        Self::GetByKey2(src)
    }
}
