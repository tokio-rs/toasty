use super::{Expr, FuncCount};

#[derive(Clone, Debug, PartialEq)]
pub enum ExprFunc {
    /// count(*)
    Count(FuncCount),
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Expr::Func(value)
    }
}
