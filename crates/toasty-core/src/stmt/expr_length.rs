use super::Expr;

/// Integer: number of elements in an array.
///
/// PostgreSQL: `cardinality(expr)`. Drives [`Path::len`](super::Path) and
/// [`Path::is_empty`](super::Path).
#[derive(Debug, Clone, PartialEq)]
pub struct ExprLength {
    /// The array whose length is being measured.
    pub expr: Box<Expr>,
}

impl Expr {
    /// Build an array-length expression (`cardinality(expr)` on PostgreSQL).
    pub fn array_length(expr: impl Into<Self>) -> Self {
        ExprLength {
            expr: Box::new(expr.into()),
        }
        .into()
    }
}

impl From<ExprLength> for Expr {
    fn from(value: ExprLength) -> Self {
        Self::Length(value)
    }
}
