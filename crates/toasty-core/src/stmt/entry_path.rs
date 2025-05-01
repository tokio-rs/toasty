use super::*;

pub trait EntryPath {
    type Iter: Iterator<Item = usize>;

    fn step_iter(self) -> Self::Iter;
}

impl EntryPath for usize {
    type Iter = std::option::IntoIter<Self>;

    fn step_iter(self) -> Self::Iter {
        Some(self).into_iter()
    }
}

impl<'a> EntryPath for &'a Projection {
    type Iter = projection::Iter<'a>;

    fn step_iter(self) -> Self::Iter {
        self.into_iter()
    }
}
