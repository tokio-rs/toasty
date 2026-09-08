use indexmap::IndexSet;
use toasty_core::{schema::db::TableId, stmt};

use crate::engine::mir;

/// Updates records by primary key.
///
/// Used with NoSQL drivers to update records given a list of primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
///
/// [`Const`]: super::Const
#[derive(Debug)]
pub(crate) struct UpdateByKey {
    /// The node producing the list of primary keys to update.
    pub(crate) input: mir::NodeId,

    /// The table to update records in.
    pub(crate) table: TableId,

    /// The field assignments to apply.
    pub(crate) assignments: stmt::Assignments,

    /// Optional additional filter applied before update.
    pub(crate) filter: Option<stmt::Expr>,

    /// Optional condition for optimistic locking.
    pub(crate) condition: Option<stmt::Expr>,

    /// The columns to return for each updated row. Empty when the update
    /// returns only an affected-row count (the type is then `Unit`).
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl From<UpdateByKey> for mir::Node {
    fn from(value: UpdateByKey) -> Self {
        mir::Operation::UpdateByKey(value).into()
    }
}
