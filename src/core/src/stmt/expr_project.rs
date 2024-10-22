use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprProject<'stmt> {
    pub base: ProjectBase<'stmt>,
    pub projection: Projection,
}

impl<'stmt> Expr<'stmt> {
    pub fn project(expr: impl Into<ExprProject<'stmt>>) -> Expr<'stmt> {
        expr.into().into()
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

impl<'stmt> ExprProject<'stmt> {
    pub const fn is_identity(&self) -> bool {
        self.base.is_expr_self() && self.projection.is_identity()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectBase<'stmt> {
    ExprSelf,
    Expr(Box<Expr<'stmt>>),
}

impl<'stmt> From<ExprProject<'stmt>> for Expr<'stmt> {
    fn from(value: ExprProject<'stmt>) -> Self {
        Expr::Project(value)
    }
}

impl<'stmt> From<&Column> for ExprProject<'stmt> {
    fn from(value: &Column) -> Self {
        ExprProject::from(value.id)
    }
}

impl<'stmt> From<ColumnId> for ExprProject<'stmt> {
    fn from(value: ColumnId) -> Self {
        ExprProject {
            base: ProjectBase::ExprSelf,
            projection: Projection::single(value.index),
        }
    }
}

impl<'stmt> From<&Field> for ExprProject<'stmt> {
    fn from(value: &Field) -> Self {
        ExprProject::from(value.id)
    }
}

impl<'stmt, T, I> From<T> for ExprProject<'stmt>
where
    T: IntoIterator<Item = I>,
    I: Into<PathStep>,
{
    fn from(value: T) -> ExprProject<'stmt> {
        ExprProject {
            base: ProjectBase::ExprSelf,
            projection: Projection::from(value),
        }
    }
}

impl ProjectBase<'_> {
    pub const fn is_expr_self(&self) -> bool {
        matches!(self, ProjectBase::ExprSelf)
    }
}
