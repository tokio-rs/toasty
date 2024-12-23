use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprArg {
    pub position: usize,
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
