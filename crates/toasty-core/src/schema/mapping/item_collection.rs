use crate::schema::{app::FieldId, db::ColumnId};

/// Item-collection metadata stored in a model's mapping.
///
/// For root models (those without `#[item_collection]`) all fields are `None`/
/// empty and the struct is effectively a no-op.  For child models the fields
/// are populated during the db-schema build phase.
#[derive(Debug, Clone, Default)]
pub struct ItemCollection {
    /// Maps this model's FK source fields to the parent model's PK fields.
    ///
    /// Populated during the db-schema build so child columns that reuse a
    /// parent's PK column can be looked up without rescanning the schema.
    pub field_mapping: indexmap::IndexMap<FieldId, FieldId>,

    /// The `__model` discriminator column shared by all models in the table.
    ///
    /// `None` for models that are not part of any item-collection group.
    /// `Some(column_id)` for both the root model and every child model.
    pub model_column: Option<ColumnId>,
}
