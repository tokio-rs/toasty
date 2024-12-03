use super::*;

pub trait EntryPath {
    type Iter: Iterator<Item = PathStep>;

    fn step_iter(self) -> Self::Iter;
}

impl EntryPath for usize {
    type Iter = std::option::IntoIter<PathStep>;

    fn step_iter(self) -> Self::Iter {
        Some(PathStep::from_usize(self)).into_iter()
    }
}

impl<'a> EntryPath for &'a Projection {
    type Iter = projection::Iter<'a>;

    fn step_iter(self) -> Self::Iter {
        self.into_iter()
    }
}
