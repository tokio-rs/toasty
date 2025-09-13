use bit_set::BitSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PathFieldSet {
    // didn't know if we wanted to use usize or the default u32
    container: BitSet<usize>,
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
        self.container.contains(val.into())
    }

    // was looking into this and the only way to keep this impl is to allocate an vec
    // tho I think it defeats the purpose of bitsets memory effficiency
    pub fn iter(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        let items: Vec<usize> = self.container.iter().collect();
        items.into_iter()
    }

    // was thinking of adding an functon like this to get the raw bitset iter?
    // pub fn iter_raw(&self) -> impl Iterator<Item = usize> + '_ {
    //     self.container.iter()
    // }

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
