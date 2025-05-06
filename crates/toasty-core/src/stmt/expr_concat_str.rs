use super::*;

#[derive(Debug, Clone)]
pub struct ExprConcatStr {
    pub exprs: Vec<Expr>,
}

impl Expr {
    pub fn concat_str(exprs: impl Into<ExprConcatStr>) -> Self {
        exprs.into().into()
    }
}

impl From<ExprConcatStr> for Expr {
    fn from(value: ExprConcatStr) -> Self {
        Self::ConcatStr(value)
    }
}

impl<T1: Into<Expr>> From<(T1,)> for ExprConcatStr {
    fn from(value: (T1,)) -> Self {
        Self {
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
        Self {
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
        Self {
            exprs: vec![value.0.into(), value.1.into(), value.2.into()],
        }
    }
}
