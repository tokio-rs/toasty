use super::{Expr, ExprFunc};

/// The SQL `COUNT` aggregate function.
///
/// When `arg` is `None`, represents `COUNT(*)` (counts all rows). When `arg` is
/// `Some`, counts the number of rows where the argument expression is non-null.
///
/// # Examples
///
/// ```text
/// COUNT(*)                       // arg: None, filter: None
/// COUNT(column)                  // arg: Some(column), filter: None
/// COUNT(*) FILTER (WHERE cond)   // arg: None, filter: Some(cond)
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FuncCount {
    /// The expression to count. `None` means `COUNT(*)`.
    pub arg: Option<Box<Expr>>,

    /// Optional filter applied before counting.
    pub filter: Option<Box<Expr>>,
}

impl Expr {
    /// Creates a `COUNT(*)` expression.
    pub fn count_star() -> Self {
        Self::Func(ExprFunc::Count(FuncCount {
            arg: None,
            filter: None,
        }))
    }
}

impl From<FuncCount> for ExprFunc {
    fn from(value: FuncCount) -> Self {
        Self::Count(value)
    }
}

impl From<FuncCount> for Expr {
    fn from(value: FuncCount) -> Self {
        Self::Func(value.into())
    }
}
