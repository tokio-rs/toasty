use crate::schema::app::FieldId;

/// Item-collection metadata stored in a model's mapping.
///
/// For models outside any item collection all fields are empty/false and the
/// struct is effectively a no-op. For roots with children and for every child,
/// `participates` is set during the db-schema build phase.
#[derive(Debug, Clone, Default)]
pub struct ItemCollection {
    /// Maps this model's FK source fields to the parent model's PK fields.
    ///
    /// Populated during the db-schema build so child columns that reuse a
    /// parent's PK column can be looked up without rescanning the schema.
    pub field_mapping: indexmap::IndexMap<FieldId, FieldId>,

    /// Whether this model participates in an item collection (root with
    /// children or any descendant). Identifies the rows whose sort column
    /// must be filtered with `<ModelName>#` so the engine emits an
    /// `IsModel` predicate at lower time and the DynamoDB driver maps that
    /// predicate to a sort-key prefix.
    pub participates: bool,
}
