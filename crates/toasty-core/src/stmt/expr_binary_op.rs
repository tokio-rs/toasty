use super::{BinaryOp, Expr};

/// A binary operation between two expressions.
///
/// Applies an operator to a left-hand side and right-hand side expression.
/// Supported operators include equality, comparison, and type checking.
///
/// # Examples
///
/// ```text
/// eq(a, b)   // a == b
/// ne(a, b)   // a != b
/// lt(a, b)   // a < b
/// gt(a, b)   // a > b
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprBinaryOp {
    /// The left-hand side expression.
    pub lhs: Box<Expr>,

    /// The operator to apply.
    pub op: BinaryOp,

    /// The right-hand side expression.
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

}

impl From<ExprBinaryOp> for Expr {
    fn from(value: ExprBinaryOp) -> Self {
        Self::BinaryOp(value)
    }
}
