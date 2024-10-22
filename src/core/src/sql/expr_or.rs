use super::*;

#[derive(Debug, Clone)]
pub struct ExprOr<'stmt> {
    pub operands: Vec<Expr<'stmt>>,
}
