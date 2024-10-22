use super::*;

#[derive(Debug, Clone)]
pub struct ExprMap<'stmt> {
    /// Expression to map
    pub base: Box<Expr<'stmt>>,

    /// How to map. This expression's self will be the result of `base`
    pub map: Box<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn map(base: impl Into<Expr<'stmt>>, map: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        ExprMap {
            base: Box::new(base.into()),
            map: Box::new(map.into()),
        }
        .into()
    }
}

impl<'stmt> From<ExprMap<'stmt>> for Expr<'stmt> {
    fn from(value: ExprMap<'stmt>) -> Self {
        Expr::Map(value)
    }
}
