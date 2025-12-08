use super::{Expr, UnaryOp};

/// A unary operation expression.
///
/// Applies a unary operator to a single operand.
///
/// # Examples
///
/// ```text
/// -x        // negation
/// -5        // negative literal
/// -(a + b)  // negation of expression
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprUnaryOp {
    /// The unary operator.
    pub op: UnaryOp,

    /// The operand expression.
    pub expr: Box<Expr>,
}

impl Expr {
    /// Creates a unary operation expression.
    pub fn unary_op(op: UnaryOp, expr: impl Into<Self>) -> Self {
        ExprUnaryOp {
            op,
            expr: Box::new(expr.into()),
        }
        .into()
    }

    /// Creates a negation expression (`-x`).
    pub fn neg(expr: impl Into<Self>) -> Self {
        Self::unary_op(UnaryOp::Neg, expr)
    }

    /// Returns true if this is a `UnaryOp` expression.
    pub fn is_unary_op(&self) -> bool {
        matches!(self, Self::UnaryOp(_))
    }
}

impl From<ExprUnaryOp> for Expr {
    fn from(value: ExprUnaryOp) -> Self {
        Self::UnaryOp(value)
    }
}
