use super::*;

#[derive(Debug, Clone)]
pub struct ExprInSubquery<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub subquery: Box<Query<'stmt>>,
}
