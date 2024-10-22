use super::*;

#[derive(Debug, Clone)]
pub struct Query<'stmt> {
    pub body: Box<ExprSet<'stmt>>,
}

impl<'stmt> Query<'stmt> {
    pub fn values(values: impl Into<Values<'stmt>>) -> Query<'stmt> {
        Query {
            body: Box::new(ExprSet::Values(values.into())),
        }
    }

    pub fn is_values(&self) -> bool {
        matches!(&*self.body, ExprSet::Values(_))
    }

    pub fn as_values(&self) -> &Values<'stmt> {
        self.try_as_values().unwrap()
    }

    pub fn try_as_values(&self) -> Option<&Values<'stmt>> {
        match &*self.body {
            ExprSet::Values(values) => Some(values),
            _ => None,
        }
    }

    pub fn as_values_mut(&mut self) -> &mut Values<'stmt> {
        self.try_as_values_mut().unwrap()
    }

    pub fn try_as_values_mut(&mut self) -> Option<&mut Values<'stmt>> {
        match &mut *self.body {
            ExprSet::Values(values) => Some(values),
            _ => None,
        }
    }

    pub fn into_values(self) -> Values<'stmt> {
        match *self.body {
            ExprSet::Values(values) => values,
            _ => panic!(),
        }
    }
}

impl<'stmt> From<Query<'stmt>> for Statement<'stmt> {
    fn from(value: Query<'stmt>) -> Self {
        Statement::Query(value)
    }
}
