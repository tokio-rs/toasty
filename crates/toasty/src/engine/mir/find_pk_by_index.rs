use indexmap::IndexSet;
use toasty_core::{
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::engine::mir;

/// Finds primary keys via a secondary index lookup.
///
/// Used with NoSQL drivers to locate records by a secondary index, returning
/// the primary keys which can then be used with [`GetByKey`](super::GetByKey).
#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    /// Nodes providing input arguments for the filter.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The table containing the index.
    pub(crate) table: TableId,

    /// The secondary index to query.
    pub(crate) index: IndexId,

    /// Filter expression for the index.
    pub(crate) filter: stmt::Expr,

    /// The return type (a list of primary keys).
    pub(crate) ty: stmt::Type,
}

impl From<FindPkByIndex> for mir::Node {
    fn from(value: FindPkByIndex) -> Self {
        mir::Operation::FindPkByIndex(value).into()
    }
}
