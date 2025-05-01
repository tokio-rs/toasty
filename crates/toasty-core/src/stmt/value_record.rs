use super::*;

use std::ops;

#[derive(Debug, Default, Clone)]
pub struct ValueRecord {
    pub fields: Vec<Value>,
}

impl ValueRecord {
    pub fn from_vec(fields: Vec<Value>) -> Self {
        Self { fields }
    }
}

impl ops::Deref for ValueRecord {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.fields[..]
    }
}

impl ops::DerefMut for ValueRecord {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fields[..]
    }
}

impl<'a> IntoIterator for &'a ValueRecord {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut ValueRecord {
    type Item = &'a mut Value;
    type IntoIter = std::slice::IterMut<'a, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl PartialEq for ValueRecord {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
