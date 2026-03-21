use super::{Field, FieldId, Schema};

/// A foreign key linking one model's fields to another model's primary key.
///
/// A `ForeignKey` is composed of one or more [`ForeignKeyField`] pairs, each
/// mapping a source field (on the owning model) to a target field (on the
/// referenced model). For single-column primary keys the `fields` vec has one
/// entry; composite keys have multiple.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{ForeignKey, ForeignKeyField, FieldId, ModelId};
///
/// let fk = ForeignKey {
///     fields: vec![ForeignKeyField {
///         source: ModelId(0).field(1),
///         target: ModelId(1).field(0),
///     }],
/// };
/// assert_eq!(fk.fields.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct ForeignKey {
    /// The field pairs that make up this foreign key.
    pub fields: Vec<ForeignKeyField>,
}

/// One column-pair within a [`ForeignKey`].
///
/// Maps a single field on the source (owning) model to the corresponding
/// field on the target (referenced) model.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::{ForeignKeyField, ModelId};
///
/// let fkf = ForeignKeyField {
///     source: ModelId(0).field(2),
///     target: ModelId(1).field(0),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct ForeignKeyField {
    /// The field on the source model that stores the foreign key value.
    pub source: FieldId,

    /// The field on the target model that this foreign key references.
    pub target: FieldId,
}

impl ForeignKey {
    pub(crate) fn is_placeholder(&self) -> bool {
        self.fields.is_empty()
    }
}

impl ForeignKeyField {
    /// Resolves the source [`Field`] from the given schema.
    pub fn source<'a>(&self, schema: &'a Schema) -> &'a Field {
        schema.field(self.source)
    }

    /// Resolves the target [`Field`] from the given schema.
    pub fn target<'a>(&self, schema: &'a Schema) -> &'a Field {
        schema.field(self.target)
    }
}
