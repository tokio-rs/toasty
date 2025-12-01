use super::Expr;

/// Tests whether an expression is null.
///
/// Returns `true` if the expression evaluates to null.
///
/// # Examples
///
/// ```text
/// is_null(x)      // returns `true` if x is null
/// is_not_null(x)  // returns `true` if x is not null
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsNull {
    /// The expression to check for null.
    pub expr: Box<Expr>,
}

impl Expr {
    pub fn is_null(expr: impl Into<Self>) -> Self {
        ExprIsNull {
            expr: Box::new(expr.into()),
        }
        .into()
    }

    pub fn is_not_null(expr: impl Into<Self>) -> Self {
        Self::not(Self::is_null(expr))
    }
}

impl From<ExprIsNull> for Expr {
    fn from(value: ExprIsNull) -> Self {
        Self::IsNull(value)
    }
}
