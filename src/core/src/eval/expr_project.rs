use super::*;

#[derive(Debug, Clone)]
pub struct ExprProject<'stmt> {
    pub base: Box<Expr<'stmt>>,
    pub projection: stmt::Projection,
}

impl<'stmt> Expr<'stmt> {
    pub fn project(
        base: impl Into<Expr<'stmt>>,
        projection: impl Into<stmt::Projection>,
    ) -> Expr<'stmt> {
        ExprProject {
            base: Box::new(base.into()),
            projection: projection.into(),
        }
        .into()
    }

    pub fn is_identity(&self) -> bool {
        todo!()
    }
}

impl<'stmt> ExprProject<'stmt> {
    pub(crate) fn from_stmt(
        expr: stmt::ExprProject<'stmt>,
        convert: &mut impl Convert<'stmt>,
    ) -> ExprProject<'stmt> {
        ExprProject {
            base: Box::new(Expr::from_stmt_by_ref(*expr.base, convert)),
            projection: expr.projection,
        }
    }
}

impl<'stmt> From<ExprProject<'stmt>> for Expr<'stmt> {
    fn from(value: ExprProject<'stmt>) -> Self {
        Expr::Project(value)
    }
}
