use super::{Expr, FuncCount};

/// A function call expression.
///
/// Represents aggregate or scalar functions applied to expressions.
///
/// # Examples
///
/// ```text
/// count(*)        // counts all rows
/// count(field)    // counts non-null values
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum ExprFunc {
    /// The `count` aggregate function.
    Count(FuncCount),
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Self::Func(value)
    }
}
