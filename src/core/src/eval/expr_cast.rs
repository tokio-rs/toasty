use super::*;

#[derive(Debug, Clone)]
pub struct ExprCast<'stmt> {
    /// The expression to cast
    pub expr: Box<Expr<'stmt>>,

    /// The type to cast it to.
    pub ty: stmt::Type,
}

impl<'stmt> Expr<'stmt> {
    pub fn cast(expr: impl Into<Expr<'stmt>>, ty: impl Into<stmt::Type>) -> Expr<'stmt> {
        ExprCast {
            expr: Box::new(expr.into()),
            ty: ty.into(),
        }
        .into()
    }
}

impl<'stmt> ExprCast<'stmt> {
    pub(crate) fn from_stmt(
        stmt: stmt::ExprCast<'stmt>,
        convert: &mut impl Convert<'stmt>,
    ) -> ExprCast<'stmt> {
        ExprCast {
            expr: Box::new(Expr::from_stmt_by_ref(*stmt.expr, convert)),
            ty: stmt.ty,
        }
    }
}

impl<'stmt> From<ExprCast<'stmt>> for Expr<'stmt> {
    fn from(value: ExprCast<'stmt>) -> Self {
        Expr::Cast(value)
    }
}
