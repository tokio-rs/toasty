use super::*;

#[derive(Debug, Clone)]
pub struct ExprProject<'stmt> {
    pub base: ProjectBase<'stmt>,
    pub projection: stmt::Projection,
}

#[derive(Debug, Clone)]
pub enum ProjectBase<'stmt> {
    ExprSelf,
    Expr(Box<Expr<'stmt>>),
}

impl<'stmt> Expr<'stmt> {
    pub fn project(expr: impl Into<ExprProject<'stmt>>) -> Expr<'stmt> {
        expr.into().into()
    }

    pub const fn identity() -> Expr<'stmt> {
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: stmt::Projection::identity(),
        })
    }

    pub const fn is_identity(&self) -> bool {
        match self {
            Expr::Project(expr_project) => expr_project.is_identity(),
            _ => false,
        }
    }
}

impl<'stmt> ExprProject<'stmt> {
    pub(crate) fn from_stmt(expr: stmt::ExprProject<'stmt>) -> ExprProject<'stmt> {
        /*
        ExprProject {
            base: match expr.base {
                stmt::ProjectBase::ExprSelf => ProjectBase::ExprSelf,
                stmt::ProjectBase::Expr(expr) => {
                    ProjectBase::Expr(Box::new(Expr::from_stmt(*expr)))
                }
            },
            projection: expr.projection.clone(),
        }
        */
        todo!()
    }

    pub const fn is_identity(&self) -> bool {
        self.base.is_expr_self() && self.projection.is_identity()
    }
}

impl ProjectBase<'_> {
    pub const fn is_expr_self(&self) -> bool {
        matches!(self, ProjectBase::ExprSelf)
    }
}

impl<'stmt, T, I> From<T> for ExprProject<'stmt>
where
    T: IntoIterator<Item = I>,
    I: Into<stmt::PathStep>,
{
    fn from(value: T) -> ExprProject<'stmt> {
        ExprProject {
            base: ProjectBase::ExprSelf,
            projection: stmt::Projection::from(value),
        }
    }
}

impl<'stmt> From<ExprProject<'stmt>> for Expr<'stmt> {
    fn from(value: ExprProject<'stmt>) -> Self {
        Expr::Project(value)
    }
}
