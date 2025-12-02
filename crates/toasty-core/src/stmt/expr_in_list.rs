use super::Expr;

/// Tests whether a value is contained in a list.
///
/// Returns `true` if `expr` equals any item in `list`. Returns `false` for an
/// empty list.
///
/// # Examples
///
/// ```text
/// in_list(x, [1, 2, 3])  // returns `true` if x is 1, 2, or 3
/// in_list(x, [])         // returns `false`
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprInList {
    /// The value to search for.
    pub expr: Box<Expr>,

    /// The list to search within.
    pub list: Box<Expr>,
}

impl Expr {
    pub fn in_list(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        ExprInList {
            expr: Box::new(lhs.into()),
            list: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprInList> for Expr {
    fn from(value: ExprInList) -> Self {
        Self::InList(value)
    }
}
