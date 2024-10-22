use super::*;

#[derive(Debug, Clone)]
pub struct ExprTuple<'stmt> {
    pub exprs: Vec<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn tuple<T>(items: impl IntoIterator<Item = T>) -> Expr<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        Expr::Tuple(ExprTuple {
            exprs: items.into_iter().map(Into::into).collect(),
        })
    }
}
