use super::{Direction, Expr};

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByExpr {
    /// The expression
    pub expr: Expr,

    /// Ascending or descending
    pub order: Option<Direction>,
}
