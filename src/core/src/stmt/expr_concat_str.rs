use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprConcatStr {
    pub exprs: Vec<Expr>,
}

impl Expr {
    pub fn concat_str(exprs: impl Into<ExprConcatStr>) -> Expr {
        exprs.into().into()
    }
}

impl From<ExprConcatStr> for Expr {
    fn from(value: ExprConcatStr) -> Self {
        Expr::ConcatStr(value)
    }
}

impl<T1: Into<Expr>> From<(T1,)> for ExprConcatStr {
    fn from(value: (T1,)) -> Self {
        ExprConcatStr {
            exprs: vec![value.0.into()],
        }
    }
}

impl<T1, T2> From<(T1, T2)> for ExprConcatStr
where
    T1: Into<Expr>,
    T2: Into<Expr>,
{
    fn from(value: (T1, T2)) -> Self {
        ExprConcatStr {
            exprs: vec![value.0.into(), value.1.into()],
        }
    }
}

impl<T1, T2, T3> From<(T1, T2, T3)> for ExprConcatStr
where
    T1: Into<Expr>,
    T2: Into<Expr>,
    T3: Into<Expr>,
{
    fn from(value: (T1, T2, T3)) -> Self {
        ExprConcatStr {
            exprs: vec![value.0.into(), value.1.into(), value.2.into()],
        }
    }
}
