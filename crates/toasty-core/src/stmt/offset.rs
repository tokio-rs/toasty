use super::Expr;

/// An offset specifying where a [`Limit`](super::Limit) starts reading rows.
///
/// Supports two strategies: count-based (`OFFSET n`) and keyset-based (start
/// after a given cursor value).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Offset, Expr, Value};
///
/// let offset = Offset::Count(Expr::from(Value::from(5_i64)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Offset {
    /// Keyset-based offset: start after the row matching this expression.
    After(Expr),

    /// Count-based offset: skip this many rows (`OFFSET n`).
    Count(Expr),
}
