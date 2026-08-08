use super::Expr;

/// Tests whether a value lies within an inclusive range.
///
/// Returns `true` if `low <= expr <= high`.
///
/// # Examples
///
/// ```text
/// between(age, 18, 65)  // returns `true` if 18 <= age <= 65
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprBetween {
    /// The value being tested.
    pub expr: Box<Expr>,
    /// Inclusive lower bound.
    pub low: Box<Expr>,
    /// Inclusive upper bound.
    pub high: Box<Expr>,
}

impl Expr {
    /// Creates a `BETWEEN` expression: `expr BETWEEN low AND high` (inclusive).
    pub fn between(expr: impl Into<Self>, low: impl Into<Self>, high: impl Into<Self>) -> Self {
        ExprBetween {
            expr: Box::new(expr.into()),
            low: Box::new(low.into()),
            high: Box::new(high.into()),
        }
        .into()
    }
}

impl From<ExprBetween> for Expr {
    fn from(value: ExprBetween) -> Self {
        Self::Between(value)
    }
}
