use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathStep(usize);

impl PathStep {
    pub const fn from_usize(index: usize) -> PathStep {
        PathStep(index)
    }

    pub const fn into_usize(self) -> usize {
        self.0
    }
}

impl<'stmt> std::ops::Index<PathStep> for [Expr<'stmt>] {
    type Output = Expr<'stmt>;

    fn index(&self, index: PathStep) -> &Self::Output {
        self.index(index.into_usize())
    }
}

impl<'stmt> std::ops::IndexMut<PathStep> for [Expr<'stmt>] {
    fn index_mut(&mut self, index: PathStep) -> &mut Self::Output {
        self.index_mut(index.into_usize())
    }
}

impl PartialEq<usize> for PathStep {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

impl From<&Field> for PathStep {
    fn from(value: &Field) -> Self {
        PathStep::from(value.id)
    }
}

impl From<FieldId> for PathStep {
    fn from(value: FieldId) -> Self {
        value.index.into()
    }
}

impl From<&FieldId> for PathStep {
    fn from(value: &FieldId) -> Self {
        value.index.into()
    }
}

impl From<ColumnId> for PathStep {
    fn from(value: ColumnId) -> Self {
        value.index.into()
    }
}

impl From<&PathStep> for PathStep {
    fn from(src: &PathStep) -> PathStep {
        *src
    }
}

impl From<usize> for PathStep {
    fn from(src: usize) -> PathStep {
        PathStep(src)
    }
}

impl From<&usize> for PathStep {
    fn from(src: &usize) -> PathStep {
        PathStep(*src)
    }
}
