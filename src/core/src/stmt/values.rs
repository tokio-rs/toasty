use super::*;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Values<'stmt> {
    pub rows: Vec<Expr<'stmt>>,
}

impl<'stmt> Values<'stmt> {
    pub fn new(rows: Vec<Expr<'stmt>>) -> Values<'stmt> {
        Values { rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
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

impl<'stmt> From<Values<'stmt>> for ExprSet<'stmt> {
    fn from(value: Values<'stmt>) -> Self {
        ExprSet::Values(value)
    }
}

impl<'stmt> From<Values<'stmt>> for Query<'stmt> {
    fn from(value: Values<'stmt>) -> Self {
        Query {
            body: Box::new(value.into()),
        }
    }
}
