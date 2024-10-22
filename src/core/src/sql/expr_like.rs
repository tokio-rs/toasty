use super::*;

#[derive(Debug, Clone)]
pub struct ExprLike<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub pattern: Box<Expr<'stmt>>,
}
