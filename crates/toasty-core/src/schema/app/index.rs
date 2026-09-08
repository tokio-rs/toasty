use super::{FieldId, ModelId};
use crate::schema::db::{IndexOp, IndexScope};

/// An index defined on a model's fields.
///
/// Indices speed up queries by letting the database locate rows without a full
/// table scan. An index can cover one or more fields, may enforce uniqueness,
/// and may serve as the primary key.
///
/// Fields are split into *partition* fields (used for distribution in NoSQL
/// backends like DynamoDB) and *local* fields (sort keys or secondary columns).
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{Index, IndexId, IndexField, ModelId};
/// use toasty_core::schema::db::{IndexOp, IndexScope};
///
/// let index = Index {
///     id: IndexId { model: ModelId(0), index: 0 },
///     name: None,
///     fields: vec![IndexField {
///         field: ModelId(0).field(0),
///         op: IndexOp::Eq,
///         scope: IndexScope::Local,
///     }],
///     unique: true,
///     primary_key: true,
/// };
/// assert!(index.unique);
/// assert!(index.primary_key);
/// ```
#[derive(Debug, Clone)]
pub struct Index {
    /// Uniquely identifies this index within the schema.
    pub id: IndexId,

    /// User-provided index name from `#[index(name = "...", ...)]` or
    /// `#[key(name = "...", ...)]`. When `None`, the schema builder generates
    /// a name automatically.
    pub name: Option<String>,

    /// Fields included in the index, in order.
    pub fields: Vec<IndexField>,

    /// When `true`, the index enforces uniqueness across indexed entries.
    pub unique: bool,

    /// When `true`, this index represents the model's primary key.
    pub primary_key: bool,
}

/// Uniquely identifies an [`Index`] within a schema.
///
/// Composed of the owning model's [`ModelId`] and a positional index into that
/// model's index list.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{IndexId, ModelId};
///
/// let id = IndexId { model: ModelId(0), index: 0 };
/// assert_eq!(id.model, ModelId(0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexId {
    /// The model this index belongs to.
    pub model: ModelId,
    /// Positional index within the model's index list.
    pub index: usize,
}

/// A single field entry within an [`Index`].
///
/// Describes which field is indexed, the comparison operation used for lookups,
/// and whether this field is a partition key or a local (sort) key.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{IndexField, ModelId};
/// use toasty_core::schema::db::{IndexOp, IndexScope};
///
/// let field = IndexField {
///     field: ModelId(0).field(1),
///     op: IndexOp::Eq,
///     scope: IndexScope::Local,
/// };
/// ```
#[derive(Debug, Copy, Clone)]
pub struct IndexField {
    /// The field being indexed.
    pub field: FieldId,

    /// The comparison operation used when querying this index field.
    pub op: IndexOp,

    /// Whether this field is a partition key or a local (sort) key.
    pub scope: IndexScope,
}
