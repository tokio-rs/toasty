use super::{Expr, ExprPattern};

/// Tests if a string expression starts with a prefix.
///
/// Returns `true` if `expr` begins with `pattern`.
///
/// # Examples
///
/// ```text
/// begins_with(name, "foo")  // returns `true` if `name` starts with "foo"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprBeginsWith {
    /// The string expression to test.
    pub expr: Box<Expr>,

    /// The prefix to match.
    pub pattern: Box<Expr>,
}

impl Expr {
    pub fn begins_with(expr: impl Into<Self>, pattern: impl Into<Self>) -> Self {
        ExprBeginsWith {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl From<ExprBeginsWith> for Expr {
    fn from(value: ExprBeginsWith) -> Self {
        Self::Pattern(value.into())
    }
}

impl From<ExprBeginsWith> for ExprPattern {
    fn from(value: ExprBeginsWith) -> Self {
        Self::BeginsWith(value)
    }
}
