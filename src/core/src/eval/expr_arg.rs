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

impl<'stmt> Expr<'stmt> {
    pub fn arg(expr_arg: impl Into<ExprArg>) -> Expr<'stmt> {
        Expr::Arg(expr_arg.into())
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

impl<'stmt> From<ExprArg> for Expr<'stmt> {
    fn from(value: ExprArg) -> Self {
        Expr::Arg(value)
    }
}
