use super::Value;

use std::{hash::Hash, ops};

#[derive(Debug, Default, Clone, Eq)]
pub struct ValueRecord {
    pub fields: Vec<Value>,
}

impl ValueRecord {
    pub fn from_vec(fields: Vec<Value>) -> Self {
        Self { fields }
    }

    pub fn as_slice(&self) -> &[Value] {
        &self[..]
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

impl IntoIterator for ValueRecord {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
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

// had to impl hash for value record because conflicting implementations of hash trait
impl Hash for ValueRecord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}
