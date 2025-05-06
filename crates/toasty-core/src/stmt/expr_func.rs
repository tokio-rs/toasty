use super::{Expr, FuncCount};

#[derive(Clone, Debug)]
pub enum ExprFunc {
    /// count(*)
    Count(FuncCount),
}

impl From<ExprFunc> for Expr {
    fn from(value: ExprFunc) -> Self {
        Self::Func(value)
    }
}
