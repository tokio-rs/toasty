use super::*;

#[derive(Debug, Clone)]
pub struct ExprOrderBy {
    /// The expression
    pub expr: Expr,

    /// Ascending or descending
    pub order: Option<Direction>,
}
