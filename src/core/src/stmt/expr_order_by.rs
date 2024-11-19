use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprOrderBy {
    /// The expression
    pub expr: Expr,

    /// Ascending or descending
    pub order: Option<Direction>,
}
