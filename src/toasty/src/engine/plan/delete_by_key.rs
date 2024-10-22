use super::*;
use crate::schema::*;

/// Input is the key to delete
#[derive(Debug)]
pub(crate) struct DeleteByKey<'stmt> {
    /// How to access input from the variable table.
    pub input: Vec<Input<'stmt>>,

    /// Which model to get
    pub table: TableId,

    /// Which keys to delete
    pub keys: eval::Expr<'stmt>,

    /// Only delete keys that match the filter
    pub filter: Option<sql::Expr<'stmt>>,
}

impl<'stmt> From<DeleteByKey<'stmt>> for Action<'stmt> {
    fn from(src: DeleteByKey<'stmt>) -> Action<'stmt> {
        Action::DeleteByKey(src)
    }
}
