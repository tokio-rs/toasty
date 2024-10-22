use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprArg {
    pub position: usize,
}

impl<'stmt> Expr<'stmt> {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Expr<'stmt> {
        Expr::Arg(expr_arg.into())
    }
}

impl From<usize> for ExprArg {
    fn from(value: usize) -> Self {
        ExprArg { position: value }
    }
}

impl<'a> From<ExprArg> for Expr<'a> {
    fn from(value: ExprArg) -> Self {
        Expr::Arg(value)
    }
}
