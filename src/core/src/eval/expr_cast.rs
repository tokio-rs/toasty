use super::*;

#[derive(Debug, Clone)]
pub struct ExprCast {
    /// The expression to cast
    pub expr: Box<Expr>,

    /// The type to cast it to.
    pub ty: stmt::Type,
}

impl Expr {
    pub fn cast(expr: impl Into<Expr>, ty: impl Into<stmt::Type>) -> Expr {
        ExprCast {
            expr: Box::new(expr.into()),
            ty: ty.into(),
        }
        .into()
    }
}

impl ExprCast {
    pub(crate) fn from_stmt(stmt: stmt::ExprCast, convert: &mut impl Convert) -> ExprCast {
        ExprCast {
            expr: Box::new(Expr::from_stmt_by_ref(*stmt.expr, convert)),
            ty: stmt.ty,
        }
    }
}

impl From<ExprCast> for Expr {
    fn from(value: ExprCast) -> Self {
        Expr::Cast(value)
    }
}
