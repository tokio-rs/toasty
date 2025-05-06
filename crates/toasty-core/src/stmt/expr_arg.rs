use super::*;

#[derive(Debug, Clone)]
pub struct ExprArg {
    pub position: usize,
}

impl Expr {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Self {
        Self::Arg(expr_arg.into())
    }

    pub fn arg_project(
        expr_arg: impl Into<ExprArg>,
        projection: impl Into<stmt::Projection>,
    ) -> Self {
        Self::project(Self::arg(expr_arg), projection)
    }
}

impl From<usize> for ExprArg {
    fn from(value: usize) -> Self {
        Self { position: value }
    }
}

impl From<ExprArg> for Expr {
    fn from(value: ExprArg) -> Self {
        Self::Arg(value)
    }
}
