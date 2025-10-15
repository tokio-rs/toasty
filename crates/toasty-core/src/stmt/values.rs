use super::{Expr, ExprSet, Query};

/// Set of values to insert
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Values {
    pub rows: Vec<Expr>,
}

impl Values {
    pub fn new(rows: Vec<Expr>) -> Self {
        Self { rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl From<Values> for ExprSet {
    fn from(value: Values) -> Self {
        Self::Values(value)
    }
}

impl From<Values> for Query {
    fn from(value: Values) -> Self {
        Self::builder(value).build()
    }
}

impl From<Expr> for Values {
    fn from(value: Expr) -> Self {
        Self { rows: vec![value] }
    }
}
