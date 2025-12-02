use super::{Expr, ExprPattern};

/// Tests if a string expression matches a SQL "like" pattern.
///
/// Returns `true` if `expr` matches `pattern` using SQL "like" semantics, where
/// `%` matches any sequence of characters and `_` matches any single character.
///
/// # Examples
///
/// ```text
/// like(name, "foo%")  // returns `true` if `name` starts with "foo"
/// like(name, "%bar")  // returns `true` if `name` ends with "bar"
/// like(name, "a_c")   // returns `true` if `name` is "abc", "adc", etc.
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprLike {
    /// The string expression to test.
    pub expr: Box<Expr>,

    /// The pattern to match against.
    pub pattern: Box<Expr>,
}

impl Expr {
    pub fn like(expr: impl Into<Self>, pattern: impl Into<Self>) -> Self {
        ExprLike {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
        }
        .into()
    }
}

impl From<ExprLike> for Expr {
    fn from(value: ExprLike) -> Self {
        Self::Pattern(value.into())
    }
}

impl From<ExprLike> for ExprPattern {
    fn from(value: ExprLike) -> Self {
        Self::Like(value)
    }
}
