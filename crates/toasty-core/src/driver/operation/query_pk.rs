use super::{Operation, Pagination};
use crate::{
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

/// Queries a table by primary key (or secondary index) with optional filtering,
/// ordering, and pagination.
///
/// This is the primary read operation for key-value drivers. The driver applies
/// `pk_filter` against the index, then applies the optional post-`filter`, and
/// returns up to `limit` rows in the requested `order`.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::operation::{QueryPk, Operation};
///
/// let op = QueryPk {
///     table: table_id,
///     index: None, // query the primary key
///     select: vec![col_a, col_b],
///     pk_filter: pk_expr,
///     filter: None,
///     limit: None,
///     order: None,
/// };
/// let operation: Operation = op.into();
/// ```
#[derive(Debug, Clone)]
pub struct QueryPk {
    /// The table to query.
    pub table: TableId,

    /// Index to query. `None` means the primary key; `Some(id)` means a
    /// secondary index.
    pub index: Option<IndexId>,

    /// Which columns to include in the returned rows.
    pub select: Vec<ColumnId>,

    /// Filter expression applied against the index key columns.
    pub pk_filter: stmt::Expr,

    /// Optional post-filter applied to rows after the index scan, before
    /// returning results to the caller.
    pub filter: Option<stmt::Expr>,

    /// Limit and pagination bounds for this query. `None` means unbounded.
    pub limit: Option<Pagination>,

    /// Sort key ordering direction for tables with a composite primary key.
    /// `None` uses the driver's default ordering.
    pub order: Option<stmt::Direction>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
