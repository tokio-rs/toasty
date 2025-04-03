use super::{Expr, ExprFunc};

#[derive(Clone, Debug, PartialEq)]
pub struct FuncCount {
    /// When `None`, it means `count(*)` Otherwise, count the number of rows for
    /// which the expression does not evaluate to `NULL`
    pub arg: Option<Box<Expr>>,

    /// Optional expression used to filter the rows before counting
    pub filter: Option<Box<Expr>>,
}

impl Expr {
    pub fn count_star() -> Expr {
        Expr::Func(ExprFunc::Count(FuncCount {
            arg: None,
            filter: None,
        }))
    }
}

impl From<FuncCount> for ExprFunc {
    fn from(value: FuncCount) -> Self {
        ExprFunc::Count(value)
    }
}

impl From<FuncCount> for Expr {
    fn from(value: FuncCount) -> Self {
        Expr::Func(value.into())
    }
}
