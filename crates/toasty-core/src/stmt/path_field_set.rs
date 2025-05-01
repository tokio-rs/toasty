use indexmap::IndexSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathFieldSet {
    // TODO: rewrite as a bitfield set
    container: IndexSet<usize>,
}

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
        let mut ret = Self::new();

        for key in iter {
            ret.container.insert(key);
        }

        ret
    }
}
