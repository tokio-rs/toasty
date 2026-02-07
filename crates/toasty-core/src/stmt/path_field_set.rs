use bit_set::BitSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathFieldSet {
    container: BitSet<u32>,
}

pub struct PathFieldSetIter<'a> {
    inner: bit_set::Iter<'a, u32>,
    len: usize,
}

impl<'a> Iterator for PathFieldSetIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.inner.next();
        if result.is_some() {
            self.len -= 1;
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> ExactSizeIterator for PathFieldSetIter<'a> {}

impl PathFieldSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_slice<T>(fields: &[T]) -> Self
    where
        for<'a> &'a T: Into<usize>,
    {
        Self {
            container: fields.iter().map(Into::into).collect(),
        }
    }

    pub fn contains(&self, val: impl Into<usize>) -> bool {
        self.container.contains(val.into())
    }

    pub fn iter(&self) -> PathFieldSetIter<'_> {
        PathFieldSetIter {
            inner: self.container.iter(),
            len: self.container.len(),
        }
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
        Self {
            container: BitSet::from_iter(iter),
        }
    }
}
