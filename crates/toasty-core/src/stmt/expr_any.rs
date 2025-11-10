use super::Expr;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprAny {
    /// Expression that evaluates to a list. Returns true if any item in the list evaluates to true.
    pub expr: Box<Expr>,
}

impl Expr {
    /// Creates an `Any` expression that returns true if any item in the list evaluates to true.
    ///
    /// Returns false if the list is empty (matching Rust's `[].iter().any()` semantics).
    pub fn any(expr: impl Into<Expr>) -> Self {
        ExprAny {
            expr: Box::new(expr.into()),
        }
        .into()
    }

    /// Returns true if this is an `Any` expression
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any(_))
    }
}

impl From<ExprAny> for Expr {
    fn from(value: ExprAny) -> Self {
        Self::Any(value)
    }
}
