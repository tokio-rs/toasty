use super::*;

use std::ops;

#[derive(Debug, Clone)]
pub enum Record<'stmt> {
    Borrowed(&'stmt [Value<'stmt>]),
    Owned(Vec<Value<'stmt>>),
}

impl<'stmt> Record<'stmt> {
    pub fn new() -> Record<'stmt> {
        Record::Owned(vec![])
    }

    pub fn from_vec(fields: Vec<Value<'stmt>>) -> Record<'stmt> {
        Record::Owned(fields)
    }

    pub fn to_fields(self) -> Vec<Value<'stmt>> {
        match self {
            Record::Borrowed(fields) => fields.to_vec(),
            Record::Owned(fields) => fields,
        }
    }

    pub fn into_owned(self) -> Record<'static> {
        Record::from_vec(match self {
            Record::Borrowed(_) => todo!(),
            Record::Owned(fields) => fields.into_iter().map(|field| field.into_owned()).collect(),
        })
    }
}

impl<'stmt> ops::Deref for Record<'stmt> {
    type Target = [Value<'stmt>];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(v) => v,
            Self::Owned(v) => v,
        }
    }
}

impl<'stmt> ops::DerefMut for Record<'stmt> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let Record::Borrowed(fields) = self {
            *self = Record::Owned(fields.to_vec());
        }

        match self {
            Record::Owned(record) => record,
            _ => unreachable!(),
        }
    }
}

impl<'a, 'stmt> IntoIterator for &'a Record<'stmt> {
    type Item = &'a Value<'stmt>;
    type IntoIter = std::slice::Iter<'a, Value<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut Record<'stmt> {
    type Item = &'a mut Value<'stmt>;
    type IntoIter = std::slice::IterMut<'a, Value<'stmt>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'stmt> PartialEq for Record<'stmt> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
