use super::*;

/// Set of values to insert
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Values {
    pub rows: Vec<Expr>,
}

impl Values {
    pub fn new(rows: Vec<Expr>) -> Values {
        Values { rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        /*
        for row in &mut self.rows {
            for item in row {
                item.substitute_ref(input);
            }
        }
        */
        todo!()
    }
}

impl From<Values> for ExprSet {
    fn from(value: Values) -> Self {
        ExprSet::Values(value)
    }
}

impl From<Values> for Query {
    fn from(value: Values) -> Self {
        Query {
            body: Box::new(value.into()),
        }
    }
}
