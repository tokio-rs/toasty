use super::Expr;

/// A `LIMIT` clause restricting the number of rows returned by a query.
///
/// Two strategies are supported:
///
/// * [`Limit::Cursor`] — keyset-based (cursor) pagination, used by
///   [`Paginate`](crate::stmt::Paginate). The engine will build pagination
///   cursors and track whether more pages exist.
/// * [`Limit::Offset`] — traditional SQL `LIMIT … OFFSET …`. No pagination
///   metadata is produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Limit {
    /// Cursor-based (keyset) pagination.
    Cursor(LimitCursor),

    /// Traditional SQL `LIMIT` with optional count-based `OFFSET`.
    Offset(LimitOffset),
}

/// Cursor-based pagination parameters.
///
/// `page_size` controls how many rows are returned per page. `after` is `None`
/// for the first page and `Some(cursor)` for subsequent pages.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitCursor {
    /// Number of rows per page.
    pub page_size: Expr,

    /// Cursor value to resume after. `None` on the first page.
    pub after: Option<Expr>,
}

/// Traditional SQL `LIMIT … OFFSET …` parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitOffset {
    /// The maximum number of rows to return.
    pub limit: Expr,

    /// Optional count-based offset (skip this many rows).
    pub offset: Option<Expr>,
}
