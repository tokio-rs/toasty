use super::*;

#[derive(Debug, Clone)]
pub struct ExprBinaryOp<'stmt> {
    pub lhs: Box<Expr<'stmt>>,
    pub op: stmt::BinaryOp,
    pub rhs: Box<Expr<'stmt>>,
}

impl<'stmt> ExprBinaryOp<'stmt> {
    pub(crate) fn from_stmt(expr: stmt::ExprBinaryOp<'stmt>) -> ExprBinaryOp<'stmt> {
        ExprBinaryOp {
            lhs: Box::new(Expr::from_stmt(*expr.lhs)),
            op: expr.op,
            rhs: Box::new(Expr::from_stmt(*expr.rhs)),
        }
    }
}

impl<'stmt> From<ExprBinaryOp<'stmt>> for Expr<'stmt> {
    fn from(value: ExprBinaryOp<'stmt>) -> Self {
        Expr::BinaryOp(value)
    }
}
