use super::Expr;

/// A string prefix-match expression: `starts_with(expr, prefix)`.
///
/// Returns `true` if `expr` starts with `prefix`. The attribute reference
/// is always `expr` (lhs) and the prefix value is always `prefix` (rhs).
///
/// # Examples
///
/// ```text
/// starts_with(name, "Al")   // true if name starts with "Al"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStartsWith {
    /// The attribute to test.
    pub expr: Box<Expr>,

    /// The prefix value to match against.
    pub prefix: Box<Expr>,
}

impl Expr {
    /// Creates a `starts_with(expr, prefix)` expression.
    pub fn starts_with(expr: impl Into<Self>, prefix: impl Into<Self>) -> Self {
        ExprStartsWith {
            expr: Box::new(expr.into()),
            prefix: Box::new(prefix.into()),
        }
        .into()
    }
}

impl From<ExprStartsWith> for Expr {
    fn from(value: ExprStartsWith) -> Self {
        Self::StartsWith(value)
    }
}
