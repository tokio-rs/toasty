use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInSubquery {
    pub expr: Box<Expr>,
    pub query: Box<Query>,
}

impl Expr {
    pub fn in_subquery(lhs: impl Into<Expr>, rhs: impl Into<Query>) -> Expr {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn is_in_subquery(&self) -> bool {
        matches!(self, Expr::InSubquery(_))
    }
}

impl From<ExprInSubquery> for Expr {
    fn from(value: ExprInSubquery) -> Self {
        Expr::InSubquery(value)
    }
}
