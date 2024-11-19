use super::*;

#[derive(Debug, Clone)]
pub struct ExprBinaryOp {
    pub lhs: Box<Expr>,
    pub op: stmt::BinaryOp,
    pub rhs: Box<Expr>,
}

impl ExprBinaryOp {
    pub(crate) fn from_stmt(expr: stmt::ExprBinaryOp, convert: &mut impl Convert) -> ExprBinaryOp {
        ExprBinaryOp {
            lhs: Box::new(Expr::from_stmt_by_ref(*expr.lhs, convert)),
            op: expr.op,
            rhs: Box::new(Expr::from_stmt_by_ref(*expr.rhs, convert)),
        }
    }
}

impl From<ExprBinaryOp> for Expr {
    fn from(value: ExprBinaryOp) -> Self {
        Expr::BinaryOp(value)
    }
}
