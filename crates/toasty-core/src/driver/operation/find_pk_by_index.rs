use super::Operation;
use crate::{
    schema::db::{IndexId, TableId},
    stmt,
};

/// Looks up primary keys through a secondary index.
///
/// The driver queries the specified secondary index using `filter` and returns
/// the matching primary key values. The query engine then uses those keys
/// in a subsequent [`GetByKey`](super::GetByKey) operation to fetch full records.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{FindPkByIndex, Operation};
///
/// let op = FindPkByIndex {
///     table: table_id,
///     index: index_id,
///     filter: filter_expr,
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct FindPkByIndex {
    /// The table that owns the index.
    pub table: TableId,

    /// The secondary index to query.
    pub index: IndexId,

    /// Filter expression applied against the index columns.
    pub filter: stmt::Expr,
}

impl From<FindPkByIndex> for Operation {
    fn from(value: FindPkByIndex) -> Self {
        Self::FindPkByIndex(value)
    }
}
