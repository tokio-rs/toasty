use indexmap::IndexSet;
use toasty_core::{schema::db::TableId, stmt};

use crate::engine::mir;

/// Batch fetches records by primary key.
///
/// Used with NoSQL drivers to retrieve multiple records given a list of
/// primary key values.
///
/// Keys are always specified as an input node, whether a [`Const`] or the
/// output of a dependent operation.
///
/// [`Const`]: super::Const
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// The node producing the list of primary keys to fetch.
    pub(crate) input: mir::NodeId,

    /// The table to fetch records from.
    pub(crate) table: TableId,

    /// The columns to include in the returned records.
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// The return type.
    pub(crate) ty: stmt::Type,
}

impl From<GetByKey> for mir::Node {
    fn from(value: GetByKey) -> Self {
        mir::Operation::GetByKey(value).into()
    }
}
