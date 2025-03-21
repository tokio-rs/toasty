use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprCast {
    /// Expression to cast
    pub expr: Box<Expr>,

    /// Type to cast to
    pub ty: Type,
}

impl Expr {
    pub fn cast(expr: impl Into<Expr>, ty: impl Into<Type>) -> Expr {
        ExprCast {
            expr: Box::new(expr.into()),
            ty: ty.into(),
        }
        .into()
    }

    pub fn is_cast(&self) -> bool {
        matches!(self, Expr::Cast(_))
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Expr::Cast(value)
    }
}
