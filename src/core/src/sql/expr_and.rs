use super::*;

#[derive(Debug, Clone)]
pub struct ExprAnd<'stmt> {
    pub operands: Vec<Expr<'stmt>>,
}
