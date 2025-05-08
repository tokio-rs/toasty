use super::OrderByExpr;

#[derive(Debug, Clone)]
pub struct OrderBy {
    pub exprs: Vec<OrderByExpr>,
}

impl From<OrderByExpr> for OrderBy {
    fn from(value: OrderByExpr) -> Self {
        Self { exprs: vec![value] }
    }
}
