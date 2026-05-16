use super::Expr;

/// Boolean: `lhs` (array) contains every element of `rhs` (array).
///
/// PostgreSQL: `lhs @> rhs`. Drives [`Path::is_superset`](super::Path).
#[derive(Debug, Clone, PartialEq)]
pub struct ExprIsSuperset {
    /// The array claimed to be the superset.
    pub lhs: Box<Expr>,
    /// The array claimed to be the subset.
    pub rhs: Box<Expr>,
}

impl Expr {
    /// Build an `IsSuperset` array predicate (`lhs @> rhs` on PostgreSQL).
    pub fn array_is_superset(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        ExprIsSuperset {
            lhs: Box::new(lhs.into()),
            rhs: Box::new(rhs.into()),
        }
        .into()
    }
}

impl From<ExprIsSuperset> for Expr {
    fn from(value: ExprIsSuperset) -> Self {
        Self::IsSuperset(value)
    }
}
