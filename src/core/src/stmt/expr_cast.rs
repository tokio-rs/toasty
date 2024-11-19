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

impl ExprCast {
    pub(crate) fn simplify(&mut self) -> Option<Expr> {
        if let Expr::Value(value) = &mut *self.expr {
            // TODO: if the unwrap fails, it is a validation bug
            let cast = self.ty.cast(value.take()).unwrap();
            Some(cast.into())
        } else {
            None
        }
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Expr::Cast(value)
    }
}
