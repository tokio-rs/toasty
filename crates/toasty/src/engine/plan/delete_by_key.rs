use crate::engine::plan::{Action, Output2, VarId};
use toasty_core::{schema::db::TableId, stmt};

/// Input is the key to delete
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// How to access input from the variable table.
    pub input: VarId,

    /// Where to store the output (impacted row count)
    pub output: Output2,

    /// Which model to get
    pub table: TableId,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,
}

impl From<DeleteByKey> for Action {
    fn from(src: DeleteByKey) -> Self {
        Self::DeleteByKey(src)
    }
}
