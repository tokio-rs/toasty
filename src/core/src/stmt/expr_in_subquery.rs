use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInSubquery {
    pub expr: Box<Expr>,
    pub query: Box<Query>,
}

impl ExprInSubquery {
    pub fn new<A, B>(expr: A, query: B) -> ExprInSubquery
    where
        A: Into<Expr>,
        B: Into<Query>,
    {
        ExprInSubquery {
            expr: Box::new(expr.into()),
            query: Box::new(query.into()),
        }
    }
}

impl From<ExprInSubquery> for Expr {
    fn from(value: ExprInSubquery) -> Self {
        Expr::InSubquery(value)
    }
}
