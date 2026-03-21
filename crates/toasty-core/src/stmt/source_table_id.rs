/// An index into a [`SourceTable`](super::SourceTable)'s `tables` vector.
///
/// Used by [`TableFactor`](super::TableFactor) and [`Join`](super::Join) to
/// reference a specific table without duplicating the full [`TableRef`](super::TableRef).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::SourceTableId;
///
/// let id = SourceTableId(0);
/// assert_eq!(id.0, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTableId(pub usize);

impl From<usize> for SourceTableId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
