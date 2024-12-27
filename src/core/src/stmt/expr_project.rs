use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprProject {
    pub base: Box<Expr>,
    pub projection: Projection,
}

impl Expr {
    pub fn project(base: impl Into<Expr>, projection: impl Into<Projection>) -> Expr {
        ExprProject {
            base: Box::new(base.into()),
            projection: projection.into(),
        }
        .into()
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Expr::Project(..))
    }

    pub fn as_project(&self) -> &ExprProject {
        match self {
            Expr::Project(expr_project) => expr_project,
            _ => panic!(),
        }
    }
}

impl ExprProject {}

impl From<ExprProject> for Expr {
    fn from(value: ExprProject) -> Self {
        Expr::Project(value)
    }
}
