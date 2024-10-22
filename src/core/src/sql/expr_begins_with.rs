use super::*;

#[derive(Debug, Clone)]
pub struct ExprBeginsWith<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub pattern: Box<Expr<'stmt>>,
}
