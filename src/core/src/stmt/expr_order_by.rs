use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprOrderBy<'stmt> {
    /// The expression
    pub expr: Expr<'stmt>,

    /// Ascending or descending
    pub order: Option<Direction>,
}
