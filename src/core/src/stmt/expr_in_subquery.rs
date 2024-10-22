use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInSubquery<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub query: Box<Query<'stmt>>,
}

impl<'stmt> ExprInSubquery<'stmt> {
    pub fn new<A, B>(expr: A, query: B) -> ExprInSubquery<'stmt>
    where
        A: Into<Expr<'stmt>>,
        B: Into<Query<'stmt>>,
    {
        ExprInSubquery {
            expr: Box::new(expr.into()),
            query: Box::new(query.into()),
        }
    }
}

impl<'stmt> From<ExprInSubquery<'stmt>> for Expr<'stmt> {
    fn from(value: ExprInSubquery<'stmt>) -> Self {
        Expr::InSubquery(value)
    }
}
