use super::Expr;

/// Tests whether an expression is null.
///
/// Returns `true` if the expression evaluates to null, or the opposite when
/// `negated` is set. The flag makes `IS NOT NULL` a predicate in its own
/// right rather than the negation of `IS NULL`: statement normalization
/// attaches variant guards to the predicate before expanding the negated
/// form to `not(is_null(..))`, so the guards end up outside the negation.
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

    /// Whether the check is `IS NOT NULL`.
    pub negated: bool,
}

impl Expr {
    /// Creates an `IS NULL` expression.
    pub fn is_null(expr: impl Into<Self>) -> Self {
        ExprIsNull {
            expr: Box::new(expr.into()),
            negated: false,
        }
        .into()
    }

    /// Creates an `IS NOT NULL` expression.
    pub fn is_not_null(expr: impl Into<Self>) -> Self {
        ExprIsNull {
            expr: Box::new(expr.into()),
            negated: true,
        }
        .into()
    }
}

impl From<ExprIsNull> for Expr {
    fn from(value: ExprIsNull) -> Self {
        Self::IsNull(value)
    }
}
