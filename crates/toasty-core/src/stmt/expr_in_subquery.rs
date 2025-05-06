use super::*;

#[derive(Debug, Clone)]
pub struct ExprInSubquery {
    pub expr: Box<Expr>,
    pub query: Box<Query>,
}

impl Expr {
    pub fn in_subquery(lhs: impl Into<Self>, rhs: impl Into<Query>) -> Self {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn is_in_subquery(&self) -> bool {
        matches!(self, Self::InSubquery(_))
    }
}

impl From<ExprInSubquery> for Expr {
    fn from(value: ExprInSubquery) -> Self {
        Self::InSubquery(value)
    }
}
