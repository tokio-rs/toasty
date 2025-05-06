use super::*;

/// Set of values to insert
#[derive(Debug, Default, Clone)]
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

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        for row in &mut self.rows {
            row.substitute_ref(input);
        }
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
