use crate::{
    stmt::{Id, Value},
    Error,
};

use std::vec;

/// Drain values from the row
pub struct IntoIter<'a> {
    inner: vec::IntoIter<Value<'a>>,
}

impl<'a> IntoIter<'a> {
    pub(crate) fn new(inner: Vec<Value<'a>>) -> IntoIter<'a> {
        IntoIter {
            inner: inner.into_iter(),
        }
    }

    pub fn next_as_bool(&mut self) -> Result<bool, Error> {
        self.try_next()?.to_bool()
    }

    pub fn next_as_id(&mut self) -> Result<Id, Error> {
        self.try_next()?.to_id()
    }

    pub fn next_as_option_id(&mut self) -> Result<Option<Id>, Error> {
        match self.try_next()? {
            Value::Null => Ok(None),
            value => value.to_id().map(Some),
        }
    }

    pub fn next_as_string(&mut self) -> Result<String, Error> {
        self.try_next()?.to_string()
    }

    pub fn next_as_option_string(&mut self) -> Result<Option<String>, Error> {
        match self.try_next()? {
            Value::Null => Ok(None),
            value => value.to_string().map(Some),
        }
    }

    pub fn next_as_i64(&mut self) -> Result<i64, Error> {
        self.try_next()?.to_i64()
    }

    fn try_next(&mut self) -> Result<Value<'a>, Error> {
        match self.next() {
            Some(value) => Ok(value),
            None => anyhow::bail!("reached end of row"),
        }
    }
}

impl<'a> Iterator for IntoIter<'a> {
    type Item = Value<'a>;

    fn next(&mut self) -> Option<Value<'a>> {
        self.inner.next()
    }
}

impl ExactSizeIterator for IntoIter<'_> {}

impl DoubleEndedIterator for IntoIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}
