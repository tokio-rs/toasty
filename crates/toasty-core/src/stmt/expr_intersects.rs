use super::Expr;

/// Boolean: `lhs` and `rhs` (both arrays) share at least one element.
///
/// PostgreSQL: `lhs && rhs`. Drives [`Path::intersects`](super::Path).
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIntersects {
    /// The first array operand.
    pub lhs: Box<Expr>,
    /// The second array operand.
    pub rhs: Box<Expr>,
}

impl Expr {
    /// Build an `Intersects` array predicate (`lhs && rhs` on PostgreSQL).
    pub fn array_intersects(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        ExprIntersects {
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprIntersects> for Expr {
    fn from(value: ExprIntersects) -> Self {
        Self::Intersects(value)
    }
}
