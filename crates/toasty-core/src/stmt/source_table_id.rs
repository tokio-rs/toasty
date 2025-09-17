#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTableId(pub usize);

impl From<usize> for SourceTableId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
