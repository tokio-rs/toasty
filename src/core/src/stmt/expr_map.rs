use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprMap {
    /// Expression to map
    pub base: Box<Expr>,

    /// How to map. This expression's self will be the result of `base`
    pub map: Box<Expr>,
}

impl Expr {
    pub fn map(base: impl Into<Expr>, map: impl Into<Expr>) -> Expr {
        ExprMap {
            base: Box::new(base.into()),
            map: Box::new(map.into()),
        }
        .into()
    }
}

impl From<ExprMap> for Expr {
    fn from(value: ExprMap) -> Self {
        Expr::Map(value)
    }
}
