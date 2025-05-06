use super::*;

#[derive(Debug, Clone)]
pub struct ExprCast {
    /// Expression to cast
    pub expr: Box<Expr>,

    /// Type to cast to
    pub ty: Type,
}

impl Expr {
    pub fn cast(expr: impl Into<Self>, ty: impl Into<Type>) -> Self {
        ExprCast {
            expr: Box::new(expr.into()),
            ty: ty.into(),
        }
        .into()
    }

    pub fn is_cast(&self) -> bool {
        matches!(self, Self::Cast(_))
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Self::Cast(value)
    }
}
