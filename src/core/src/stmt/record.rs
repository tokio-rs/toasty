mod into_iter;
pub use into_iter::IntoIter;

use crate::stmt::Value;

use std::ops;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Record<'stmt> {
    fields: Vec<Value<'stmt>>,
}

impl<'stmt> Record<'stmt> {
    pub fn new(width: usize) -> Record<'stmt> {
        Record::from_vec(vec![Value::Null; width])
    }

    pub fn from_vec(fields: Vec<Value<'stmt>>) -> Record<'stmt> {
        Record { fields }
    }

    /// Consume a value.
    ///
    /// Attempts to get the same field in the future will return `None`.
    pub fn take(&mut self, index: usize) -> Value<'stmt> {
        std::mem::take(&mut self.fields[index])
    }

    pub fn fields(&self) -> impl ExactSizeIterator<Item = &Value<'stmt>> {
        self.fields.iter()
    }

    pub fn into_fields(self) -> IntoIter<'stmt> {
        self.into_iter()
    }

    pub fn to_static<'b>(&self) -> Record<'b> {
        todo!("self={:#?}", self)
    }

    pub fn into_owned(self) -> Record<'static> {
        Record {
            fields: self.fields.into_iter().map(Value::into_owned).collect(),
        }
    }
}

impl<'stmt> ops::Deref for Record<'stmt> {
    type Target = [Value<'stmt>];

    fn deref(&self) -> &Self::Target {
        self.fields.deref()
    }
}

impl<'stmt> ops::DerefMut for Record<'stmt> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.fields.deref_mut()
    }
}

impl<'stmt> IntoIterator for Record<'stmt> {
    type Item = Value<'stmt>;
    type IntoIter = IntoIter<'stmt>;

    fn into_iter(self) -> IntoIter<'stmt> {
        IntoIter::new(self.fields)
    }
}

impl<'a, 'stmt> IntoIterator for &'a Record<'stmt> {
    type Item = &'a Value<'stmt>;
    type IntoIter = std::slice::Iter<'a, Value<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.iter()
    }
}
