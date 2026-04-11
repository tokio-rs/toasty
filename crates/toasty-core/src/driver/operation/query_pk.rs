use super::Operation;
use crate::{
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

/// Describes how results from a [`QueryPk`] operation should be bounded.
#[derive(Debug, Clone)]
pub enum QueryPkLimit {
    /// Cursor-based (keyset) pagination. Returns `page_size` items resuming
    /// after `after`. `after = None` means the first page.
    Cursor {
        /// Maximum number of items to return per page.
        page_size: i64,
        /// Serialized key of the last item from the previous page, or `None`
        /// for the first page.
        after: Option<stmt::Value>,
    },
    /// Hard-limit with optional client-side skip. Returns up to `limit` items
    /// after discarding the first `offset`.
    Offset {
        /// Maximum number of items to return.
        limit: i64,
        /// Number of leading items to skip before returning results.
        offset: Option<i64>,
    },
}

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
///     pagination: None,
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

    /// Pagination bounds for this query. `None` means no limit or cursor.
    pub pagination: Option<QueryPkLimit>,

    /// Sort key ordering direction for tables with a composite primary key.
    /// `None` uses the driver's default ordering.
    pub order: Option<stmt::Direction>,
}

impl From<QueryPk> for Operation {
    fn from(value: QueryPk) -> Self {
        Self::QueryPk(value)
    }
}
