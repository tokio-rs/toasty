use super::*;

#[derive(Debug, Clone)]
pub struct ExprArg {
    pub position: usize,
}

impl ExprArg {
    pub(crate) fn from_stmt(stmt: stmt::ExprArg) -> ExprArg {
        ExprArg {
            position: stmt.position,
        }
    }
}

impl Expr {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Expr {
        Expr::Arg(expr_arg.into())
    }

    pub fn arg_project(
        expr_arg: impl Into<ExprArg>,
        projection: impl Into<stmt::Projection>,
    ) -> Expr {
        Expr::project(Expr::arg(expr_arg), projection)
    }

    pub fn is_arg(&self) -> bool {
        matches!(self, Expr::Arg(..))
    }
}

impl From<usize> for ExprArg {
    fn from(value: usize) -> Self {
        ExprArg { position: value }
    }
}

impl From<ExprArg> for Expr {
    fn from(value: ExprArg) -> Self {
        Expr::Arg(value)
    }
}
