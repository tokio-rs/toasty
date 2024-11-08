use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprCast<'stmt> {
    /// Expression to cast
    pub expr: Box<Expr<'stmt>>,

    /// Type to cast to
    pub ty: Type,
}

impl<'stmt> Expr<'stmt> {
    pub fn cast(expr: impl Into<Expr<'stmt>>, ty: impl Into<Type>) -> Expr<'stmt> {
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

impl<'stmt> ExprCast<'stmt> {
    pub(crate) fn simplify(&mut self) -> Option<Expr<'stmt>> {
        if let Expr::Value(value) = &mut *self.expr {
            // TODO: if the unwrap fails, it is a validation bug
            let cast = self.ty.cast(value.take()).unwrap();
            Some(cast.into())
        } else {
            None
        }
    }
}

impl<'stmt> From<ExprCast<'stmt>> for Expr<'stmt> {
    fn from(value: ExprCast<'stmt>) -> Self {
        Expr::Cast(value)
    }
}
