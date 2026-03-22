use super::{FieldId, IndexId};

/// The primary key definition for a root model.
///
/// A primary key consists of one or more fields that uniquely identify a
/// record, plus a reference to the backing [`Index`](super::Index).
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{PrimaryKey, IndexId, ModelId};
///
/// let pk = PrimaryKey {
///     fields: vec![ModelId(0).field(0)],
///     index: IndexId { model: ModelId(0), index: 0 },
/// };
/// assert_eq!(pk.fields.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct PrimaryKey {
    /// The fields that compose this primary key, in order.
    pub fields: Vec<FieldId>,

    /// The [`IndexId`] of the index backing this primary key.
    pub index: IndexId,
}
