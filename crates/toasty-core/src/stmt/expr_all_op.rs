use super::{BinaryOp, Expr};

/// `lhs <op> ALL(rhs)` predicate against an array-valued operand.
///
/// Evaluates to `true` only if `lhs <op> item` holds for every `item` in
/// `rhs`. Modeled on `sqlparser-rs`'s `AllOp`. Toasty currently lowers
/// `NOT IN (...)` to this with [`BinaryOp::Ne`], but the operator is carried
/// separately so the shape generalizes to other comparisons.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprAllOp {
    /// The scalar operand on the left.
    pub lhs: Box<Expr>,

    /// The comparison operator applied between `lhs` and each element of `rhs`.
    pub op: BinaryOp,

    /// The array-typed operand on the right (typically `Expr::Arg(n)` of
    /// `Type::List(elem)`).
    pub rhs: Box<Expr>,
}

impl Expr {
    /// Creates a `lhs <op> ALL(rhs)` expression.
    pub fn all_op(lhs: impl Into<Self>, op: BinaryOp, rhs: impl Into<Self>) -> Self {
        ExprAllOp {
            lhs: Box::new(lhs.into()),
            op,
            rhs: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprAllOp> for Expr {
    fn from(value: ExprAllOp) -> Self {
        Self::AllOp(value)
    }
}
