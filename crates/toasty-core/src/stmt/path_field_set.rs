use bit_set::BitSet;
use std::ops::{BitAnd, BitOr, BitOrAssign};

/// A set of field indices, backed by a bit set.
///
/// Used to track which fields are present in a [`SparseRecord`](super::SparseRecord)
/// or which fields are part of a type description. Supports set operations
/// like union (`|`), intersection (`&`), and membership tests.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::PathFieldSet;
///
/// let mut set = PathFieldSet::new();
/// set.insert(0);
/// set.insert(2);
/// assert!(set.contains(0_usize));
/// assert!(!set.contains(1_usize));
/// assert_eq!(set.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathFieldSet {
    container: BitSet<u32>,
}

/// An iterator over the field indices in a [`PathFieldSet`].
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
    /// Creates an empty field set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a field set from a slice of values convertible to `usize`.
    pub fn from_slice<T>(fields: &[T]) -> Self
    where
        for<'a> &'a T: Into<usize>,
    {
        Self {
            container: fields.iter().map(Into::into).collect(),
        }
    }

    /// Returns `true` if the set contains the given field index.
    pub fn contains(&self, val: impl Into<usize>) -> bool {
        self.container.contains(val.into())
    }

    /// Returns an iterator over the field indices in ascending order.
    pub fn iter(&self) -> PathFieldSetIter<'_> {
        PathFieldSetIter {
            inner: self.container.iter(),
            len: self.container.len(),
        }
    }

    /// Returns `true` if the set contains no field indices.
    pub fn is_empty(&self) -> bool {
        self.container.is_empty()
    }

    /// Returns the number of field indices in the set.
    pub fn len(&self) -> usize {
        self.container.len()
    }

    /// Inserts a field index into the set.
    pub fn insert(&mut self, val: usize) {
        self.container.insert(val);
    }
}

impl BitOr for PathFieldSet {
    type Output = Self;

    fn bitor(mut self, rhs: Self) -> Self {
        self.container.union_with(&rhs.container);
        self
    }
}

impl BitOrAssign for PathFieldSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.container.union_with(&rhs.container);
    }
}

impl BitAnd for PathFieldSet {
    type Output = Self;

    fn bitand(mut self, rhs: Self) -> Self {
        self.container.intersect_with(&rhs.container);
        self
    }
}

impl FromIterator<usize> for PathFieldSet {
    fn from_iter<T: IntoIterator<Item = usize>>(iter: T) -> Self {
        Self {
            container: BitSet::from_iter(iter),
        }
    }
}
