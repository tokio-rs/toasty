use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathFieldSet {
    // TODO: rewrite as a bitfield set
    container: HashSet<usize>,
}

impl PathFieldSet {
    pub fn new() -> PathFieldSet {
        PathFieldSet::default()
    }

    pub fn from_slice<T>(fields: &[T]) -> PathFieldSet
    where
        for<'a> &'a T: Into<usize>,
    {
        PathFieldSet {
            container: fields.iter().map(Into::into).collect(),
        }
    }

    pub fn insert(&mut self, val: impl Into<usize>) {
        self.container.insert(val.into());
    }

    pub fn unset(&mut self, field: impl Into<usize>) {
        self.container.remove(&field.into());
    }

    pub fn contains(&self, val: impl Into<usize>) -> bool {
        self.container.contains(&val.into())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        self.container.iter().map(Clone::clone)
    }

    pub fn is_empty(&self) -> bool {
        self.container.is_empty()
    }

    pub fn len(&self) -> usize {
        self.container.len()
    }
}

impl FromIterator<usize> for PathFieldSet {
    fn from_iter<T: IntoIterator<Item = usize>>(iter: T) -> Self {
        let mut ret = PathFieldSet::new();

        for key in iter {
            ret.insert(key);
        }

        ret
    }
}
