use super::*;

use crate::schema::db::TableId;

/// Input is the key to delete
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// How to access input from the variable table.
    pub input: Option<Input>,

    /// Which model to get
    pub table: TableId,

    /// Which keys to delete
    pub keys: eval::Func,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,
}

impl From<DeleteByKey> for Action {
    fn from(src: DeleteByKey) -> Self {
        Self::DeleteByKey(src)
    }
}
