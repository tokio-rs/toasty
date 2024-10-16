use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprProject<'stmt> {
    pub base: Box<Expr<'stmt>>,
    pub projection: Projection,
}

impl<'stmt> Expr<'stmt> {
    pub fn project(base: impl Into<Expr<'stmt>>, projection: impl Into<Projection>) -> Expr<'stmt> {
        ExprProject {
            base: Box::new(base.into()),
            projection: projection.into(),
        }
        .into()
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Expr::Project(..))
    }

    pub fn as_project(&self) -> &ExprProject<'stmt> {
        match self {
            Expr::Project(expr_project) => expr_project,
            _ => panic!(),
        }
    }
}

impl<'stmt> ExprProject<'stmt> {}

impl<'stmt> From<ExprProject<'stmt>> for Expr<'stmt> {
    fn from(value: ExprProject<'stmt>) -> Self {
        Expr::Project(value)
    }
}
