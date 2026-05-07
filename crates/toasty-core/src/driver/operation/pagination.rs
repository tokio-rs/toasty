use crate::stmt;

/// Describes how results from a paged operation should be bounded.
///
/// Used by both [`QueryPk`](super::QueryPk) and [`Scan`](super::Scan).
#[derive(Debug, Clone)]
pub enum Pagination {
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
