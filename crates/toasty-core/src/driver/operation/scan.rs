use super::{Operation, Pagination};
use crate::{schema::db::TableId, stmt};

/// A full-table scan operation.
///
/// Sent to drivers that set [`Capability::scan`](crate::driver::Capability::scan) to `true`
/// (currently only DynamoDB). The driver scans the entire table and applies
/// `filter` to each row before returning results.
#[derive(Debug, Clone)]
pub struct Scan {
    /// Table to scan.
    pub table: TableId,

    /// Column indices to return (relative to the table's column list).
    pub columns: Vec<usize>,

    /// Optional filter expression applied to each row after scanning.
    pub filter: Option<stmt::Expr>,

    /// Limit and pagination bounds. `None` means return all rows.
    ///
    /// - `Cursor` for keyset/cursor-based pagination (`.paginate()`)
    /// - `Offset` for hard-limit with optional skip (`.limit()` / `.offset()`)
    pub limit: Option<Pagination>,
}

impl From<Scan> for Operation {
    fn from(value: Scan) -> Self {
        Self::Scan(value)
    }
}
