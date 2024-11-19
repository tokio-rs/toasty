use super::*;

#[derive(Debug, Clone)]
pub struct ExprProject {
    pub base: Box<Expr>,
    pub projection: stmt::Projection,
}

impl Expr {
    pub fn project(base: impl Into<Expr>, projection: impl Into<stmt::Projection>) -> Expr {
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

impl ExprProject {
    pub(crate) fn from_stmt(expr: stmt::ExprProject, convert: &mut impl Convert) -> ExprProject {
        ExprProject {
            base: Box::new(Expr::from_stmt_by_ref(*expr.base, convert)),
            projection: expr.projection,
        }
    }
}

impl From<ExprProject> for Expr {
    fn from(value: ExprProject) -> Self {
        Expr::Project(value)
    }
}
