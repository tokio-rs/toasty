use super::*;

use std::ops;

#[derive(Debug, Clone)]
pub enum RecordCow<'stmt> {
    Borrowed(&'stmt Record<'stmt>),
    Owned(Record<'stmt>),
}

impl<'stmt> RecordCow<'stmt> {
    pub fn into_owned(self) -> Record<'stmt> {
        match self {
            RecordCow::Borrowed(record) => record.clone(),
            RecordCow::Owned(record) => record,
        }
    }

    pub fn into_static(self) -> RecordCow<'static> {
        RecordCow::Owned(self.to_static())
    }
}

impl<'stmt> ops::Deref for RecordCow<'stmt> {
    type Target = Record<'stmt>;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(v) => v,
            Self::Owned(v) => v,
        }
    }
}

impl<'stmt> ops::DerefMut for RecordCow<'stmt> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if let RecordCow::Borrowed(record) = self {
            *self = RecordCow::Owned(record.clone());
        }

        match self {
            RecordCow::Owned(record) => record,
            _ => unreachable!(),
        }
    }
}

impl<'stmt> PartialEq for RecordCow<'stmt> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<'stmt> PartialEq<Record<'stmt>> for RecordCow<'stmt> {
    fn eq(&self, other: &Record<'stmt>) -> bool {
        **self == *other
    }
}

impl<'stmt> From<&'stmt Record<'stmt>> for RecordCow<'stmt> {
    fn from(value: &'stmt Record<'stmt>) -> Self {
        RecordCow::Borrowed(value)
    }
}

impl<'stmt> From<Record<'stmt>> for RecordCow<'stmt> {
    fn from(value: Record<'stmt>) -> Self {
        RecordCow::Owned(value)
    }
}
