use super::OrderByExpr;

#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub exprs: Vec<OrderByExpr>,
}

impl From<OrderByExpr> for OrderBy {
    fn from(value: OrderByExpr) -> Self {
        Self { exprs: vec![value] }
    }
}
