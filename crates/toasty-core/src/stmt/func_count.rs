use super::{Expr, ExprFunc};

#[derive(Clone, Debug)]
pub struct FuncCount {
    /// When `None`, it means `count(*)` Otherwise, count the number of rows for
    /// which the expression does not evaluate to `NULL`
    pub arg: Option<Box<Expr>>,

    /// Optional expression used to filter the rows before counting
    pub filter: Option<Box<Expr>>,
}

impl Expr {
    pub fn count_star() -> Self {
        Self::Func(ExprFunc::Count(FuncCount {
            arg: None,
            filter: None,
        }))
    }
}

impl From<FuncCount> for ExprFunc {
    fn from(value: FuncCount) -> Self {
        Self::Count(value)
    }
}

impl From<FuncCount> for Expr {
    fn from(value: FuncCount) -> Self {
        Self::Func(value.into())
    }
}
