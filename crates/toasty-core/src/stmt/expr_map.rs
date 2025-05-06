use super::*;

#[derive(Debug, Clone)]
pub struct ExprMap {
    /// Expression to map
    pub base: Box<Expr>,

    /// How to map. This expression's self will be the result of `base`
    pub map: Box<Expr>,
}

impl Expr {
    pub fn map(base: impl Into<Self>, map: impl Into<Self>) -> Self {
        ExprMap {
            base: Box::new(base.into()),
            map: Box::new(map.into()),
        }
        .into()
    }

    pub fn as_map(&self) -> &ExprMap {
        match self {
            Self::Map(expr) => expr,
            _ => todo!(),
        }
    }
}

impl From<ExprMap> for Expr {
    fn from(value: ExprMap) -> Self {
        Self::Map(value)
    }
}
