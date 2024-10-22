use super::*;

#[derive(Debug, Clone)]
pub struct ExprBinaryOp<'stmt> {
    pub lhs: Box<Expr<'stmt>>,
    pub op: BinaryOp,
    pub rhs: Box<Expr<'stmt>>,
}

impl<'stmt> From<ExprBinaryOp<'stmt>> for Expr<'stmt> {
    fn from(value: ExprBinaryOp<'stmt>) -> Self {
        Expr::BinaryOp(value)
    }
}
