use super::OrderByExpr;

#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub exprs: Vec<OrderByExpr>,
}

impl OrderBy {
    /// Flips the direction of each [`OrderByExpr`] that makes up this [`OrderBy`].
    pub fn reverse(&mut self) {
        for expr in &mut self.exprs {
            expr.reverse();
        }
    }
}

impl From<OrderByExpr> for OrderBy {
    fn from(value: OrderByExpr) -> Self {
        Self { exprs: vec![value] }
    }
}
