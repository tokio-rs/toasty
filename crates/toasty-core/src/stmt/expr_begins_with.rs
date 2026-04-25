use super::Expr;

/// A string prefix-match expression: `begins_with(expr, prefix)`.
///
/// Returns `true` if `expr` starts with `prefix`. The attribute reference
/// is always `expr` (lhs) and the prefix value is always `prefix` (rhs).
///
/// # Examples
///
/// ```text
/// begins_with(name, "Al")   // true if name starts with "Al"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprBeginsWith {
    /// The attribute to test.
    pub expr: Box<Expr>,

    /// The prefix value to match against.
    pub prefix: Box<Expr>,
}

impl Expr {
    /// Creates a `begins_with(expr, prefix)` expression.
    pub fn begins_with(expr: impl Into<Self>, prefix: impl Into<Self>) -> Self {
        ExprBeginsWith {
            expr: Box::new(expr.into()),
            prefix: Box::new(prefix.into()),
        }
        .into()
    }
}

impl From<ExprBeginsWith> for Expr {
    fn from(value: ExprBeginsWith) -> Self {
        Self::BeginsWith(value)
    }
}
