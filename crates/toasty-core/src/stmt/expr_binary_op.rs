use super::*;

#[derive(Debug, Clone)]
pub struct ExprBinaryOp {
    pub lhs: Box<Expr>,
    pub op: BinaryOp,
    pub rhs: Box<Expr>,
}

impl Expr {
    pub fn binary_op(lhs: impl Into<Self>, op: BinaryOp, rhs: impl Into<Self>) -> Self {
        ExprBinaryOp {
            op,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn eq(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Eq, rhs)
    }

    /// Returns true if the expression is a binary expression with the equality operator
    pub fn is_eq(&self) -> bool {
        matches!(
            self,
            Self::BinaryOp(ExprBinaryOp {
                op: BinaryOp::Eq,
                ..
            })
        )
    }

    pub fn ge(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Ge, rhs)
    }

    pub fn gt(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Gt, rhs)
    }

    pub fn le(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Le, rhs)
    }

    pub fn lt(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Lt, rhs)
    }

    pub fn ne(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::Ne, rhs)
    }

    pub fn is_a(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        Expr::binary_op(lhs, BinaryOp::IsA, rhs)
    }
}

impl From<ExprBinaryOp> for Expr {
    fn from(value: ExprBinaryOp) -> Self {
        Self::BinaryOp(value)
    }
}
