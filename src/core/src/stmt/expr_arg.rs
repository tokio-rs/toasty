use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprArg {
    pub position: usize,
}

impl Expr {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Expr {
        Expr::Arg(expr_arg.into())
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
