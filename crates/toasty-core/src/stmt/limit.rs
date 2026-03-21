use super::{Expr, Offset};

/// A `LIMIT` clause with an optional offset.
///
/// Restricts the number of rows returned by a query. The limit is an
/// expression (typically a constant integer). An optional [`Offset`] specifies
/// where to start (either count-based or keyset-based).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Limit, Expr, Value};
///
/// let limit = Limit {
///     limit: Expr::from(Value::from(10_i64)),
///     offset: None,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    /// The maximum number of rows to return.
    pub limit: Expr,

    /// Optional offset (where to start).
    pub offset: Option<Offset>,
}
