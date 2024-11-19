use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprBinaryOp {
    pub lhs: Box<Expr>,
    pub op: BinaryOp,
    pub rhs: Box<Expr>,
}

impl Expr {
    pub fn eq(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Eq,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    /// Returns true if the expression is a binary expression with the equality operator
    pub fn is_eq(&self) -> bool {
        matches!(
            self,
            Expr::BinaryOp(ExprBinaryOp {
                op: BinaryOp::Eq,
                ..
            })
        )
    }

    pub fn ge(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Ge,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn gt(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Gt,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn le(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Le,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn lt(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Lt,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn ne(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::Ne,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn is_a(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprBinaryOp {
            op: BinaryOp::IsA,
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }
}

impl ExprBinaryOp {
    pub(crate) fn simplify(&mut self) -> Option<Expr> {
        match (&mut *self.lhs, &mut *self.rhs) {
            (Expr::Cast(lhs), Expr::Value(Value::Id(rhs))) if lhs.ty.is_id() => {
                // TODO: don't clone
                *self.lhs = (*lhs.expr).clone();
                *self.rhs = rhs.to_primitive().into();
            }
            _ => {}
        }

        None
    }
}

impl From<ExprBinaryOp> for Expr {
    fn from(value: ExprBinaryOp) -> Self {
        Expr::BinaryOp(value)
    }
}
