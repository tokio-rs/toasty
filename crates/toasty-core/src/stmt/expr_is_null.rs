use super::Expr;

/// Tests whether an expression is null.
///
/// Returns `true` if the expression evaluates to null (or not null when
/// negated).
///
/// # Examples
///
/// ```text
/// is_null(x)      // returns `true` if x is null
/// is_not_null(x)  // returns `true` if x is not null
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsNull {
    /// When `true`, this is an "is not null" check.
    pub negate: bool,

    /// The expression to check for null.
    pub expr: Box<Expr>,
}

impl Expr {
    pub fn is_null(expr: impl Into<Self>) -> Self {
        ExprIsNull {
            negate: false,
            expr: Box::new(expr.into()),
        }
        .into()
    }

    pub fn is_not_null(expr: impl Into<Self>) -> Self {
        ExprIsNull {
            negate: true,
            expr: Box::new(expr.into()),
        }
        .into()
    }
}

impl From<ExprIsNull> for Expr {
    fn from(value: ExprIsNull) -> Self {
        Self::IsNull(value)
    }
}
