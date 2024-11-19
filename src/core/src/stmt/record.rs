use super::*;

use std::ops;

#[derive(Debug, Clone)]
pub struct Record {
    pub fields: Vec<Value>,
}

impl Record {
    pub fn new() -> Record {
        Record { fields: vec![] }
    }

    pub fn from_vec(fields: Vec<Value>) -> Record {
        Record { fields }
    }
}

impl ops::Deref for Record {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.fields[..]
    }
}

impl ops::DerefMut for Record {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields[..]
    }
}

impl<'a> IntoIterator for &'a Record {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Record {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
