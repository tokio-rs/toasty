use super::{BinaryOp, Expr};

/// `lhs <op> ANY(rhs)` predicate against an array-valued operand.
///
/// Evaluates to `true` if `lhs <op> item` holds for any `item` in `rhs`.
/// Modeled on `sqlparser-rs`'s `AnyOp`. Toasty currently lowers `IN (...)` to
/// this with [`BinaryOp::Eq`], but the operator is carried separately so the
/// shape generalizes to other comparisons.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprAnyOp {
    /// The scalar operand on the left.
    pub lhs: Box<Expr>,

    /// The comparison operator applied between `lhs` and each element of `rhs`.
    pub op: BinaryOp,

    /// The array-typed operand on the right (typically `Expr::Arg(n)` of
    /// `Type::List(elem)`).
    pub rhs: Box<Expr>,
}

impl Expr {
    /// Creates a `lhs <op> ANY(rhs)` expression.
    pub fn any_op(lhs: impl Into<Self>, op: BinaryOp, rhs: impl Into<Self>) -> Self {
        ExprAnyOp {
            lhs: Box::new(lhs.into()),
            op,
            rhs: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprAnyOp> for Expr {
    fn from(value: ExprAnyOp) -> Self {
        Self::AnyOp(value)
    }
}
