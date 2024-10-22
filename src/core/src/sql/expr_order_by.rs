use super::*;

#[derive(Debug, Clone)]
pub struct ExprOrderBy<'stmt> {
    /// The expression
    pub expr: Expr<'stmt>,

    /// Ascending or descending
    pub order: Option<Direction>,
}
