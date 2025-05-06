use super::*;

#[derive(Debug, Clone)]
pub struct ExprProject {
    pub base: Box<Expr>,
    pub projection: Projection,
}

impl Expr {
    pub fn project(base: impl Into<Self>, projection: impl Into<Projection>) -> Self {
        ExprProject {
            base: Box::new(base.into()),
            projection: projection.into(),
        }
        .into()
    }

    pub fn is_project(&self) -> bool {
        matches!(self, Self::Project(..))
    }

    pub fn as_project(&self) -> &ExprProject {
        match self {
            Self::Project(expr_project) => expr_project,
            _ => panic!(),
        }
    }
}

impl ExprProject {}

impl From<ExprProject> for Expr {
    fn from(value: ExprProject) -> Self {
        Self::Project(value)
    }
}
