use toasty_core::{schema::db::TableId, stmt};

use crate::engine::mir;

/// Deletes records by primary key.
///
/// Used with NoSQL drivers to delete records given a list of primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
///
/// [`Const`]: super::Const
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// The node producing the list of primary keys to delete.
    pub(crate) input: mir::NodeId,

    /// The table to delete records from.
    pub(crate) table: TableId,

    /// Optional additional filter applied before deletion.
    pub(crate) filter: Option<stmt::Expr>,

    /// Optional condition for optimistic locking (e.g., version check).
    pub(crate) condition: Option<stmt::Expr>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl From<DeleteByKey> for mir::Node {
    fn from(value: DeleteByKey) -> Self {
        mir::Operation::DeleteByKey(value).into()
    }
}
