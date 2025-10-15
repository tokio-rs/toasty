use crate::stmt::{Expr, Query};

#[derive(Debug, Clone, PartialEq)]
pub struct ExprExists {
    pub subquery: Box<Query>,
    pub negated: bool,
}

impl Expr {
    pub fn exists(subquery: impl Into<Query>) -> Expr {
        Expr::Exists(ExprExists {
            subquery: Box::new(subquery.into()),
            negated: false,
        })
    }

    pub fn not_exists(subquery: impl Into<Query>) -> Expr {
        Expr::Exists(ExprExists {
            subquery: Box::new(subquery.into()),
            negated: true,
        })
    }
}

impl From<ExprExists> for Expr {
    fn from(value: ExprExists) -> Self {
        Self::Exists(value)
    }
}
