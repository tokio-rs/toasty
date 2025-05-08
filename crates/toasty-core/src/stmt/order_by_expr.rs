use super::*;

#[derive(Debug, Clone)]
pub struct OrderByExpr {
    /// The expression
    pub expr: Expr,

    /// Ascending or descending
    pub order: Option<Direction>,
}
