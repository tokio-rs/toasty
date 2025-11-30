use super::Expr;

/// Negates a boolean expression.
///
/// Returns `true` if the inner expression evaluates to `false`, and vice versa.
/// Returns `NULL` if the inner expression evaluates to `NULL`.
///
/// # Examples
///
/// ```text
/// not(true)   // returns `false`
/// not(false)  // returns `true`
/// not(null)   // returns `null`
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprNot {
    /// The expression to negate.
    pub expr: Box<Expr>,
}

impl Expr {
    /// Creates a `Not` expression that negates the given expression.
    pub fn not(expr: impl Into<Self>) -> Self {
        ExprNot {
            expr: Box::new(expr.into()),
        }
        .into()
    }

    /// Returns true if this is a `Not` expression.
    pub fn is_not(&self) -> bool {
        matches!(self, Self::Not(_))
    }
}

impl From<ExprNot> for Expr {
    fn from(value: ExprNot) -> Self {
        Self::Not(value)
    }
}
