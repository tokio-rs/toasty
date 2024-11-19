use super::*;
use crate::schema::*;

/// Input is the key to delete
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// How to access input from the variable table.
    pub input: Vec<Input>,

    /// Which model to get
    pub table: TableId,

    /// Which keys to delete
    pub keys: eval::Expr,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,
}

impl From<DeleteByKey> for Action {
    fn from(src: DeleteByKey) -> Action {
        Action::DeleteByKey(src)
    }
}
