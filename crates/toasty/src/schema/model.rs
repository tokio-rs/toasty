use super::*;
use toasty_core::schema::{db, Name};

/// Represents a model's schema as known at macro compilation time.
///
/// This is the "incomplete" version of `toasty_core::schema::app::Model` that contains only
/// information available to the macro. Notably missing:
/// - ModelId (not assigned until schema registration)
/// - FieldId references (depend on ModelId)
/// - Resolved relation pairs (require cross-model analysis)
#[derive(Debug, Clone)]
pub struct Model {
    /// TypeId of the model type (used for relation resolution)
    pub type_id: std::any::TypeId,

    /// Name of the model
    pub name: Name,

    /// Fields contained by the model (with unresolved references)
    pub fields: Vec<Field>,

    /// Primary key field indices (within this model's fields)
    pub primary_key: PrimaryKey,

    /// Index definitions
    pub indices: Vec<Index>,

    /// If the schema specifies a table to map the model to, this is set.
    pub table_name: Option<String>,
}

/// Primary key definition using field indices instead of FieldIds
#[derive(Debug, Clone)]
pub struct PrimaryKey {
    /// Indices of fields that make up the primary key
    pub fields: Vec<usize>,
}

/// Index definition using field indices instead of FieldIds
#[derive(Debug, Clone)]
pub struct Index {
    /// Field indices that make up this index
    pub fields: Vec<IndexField>,

    /// Whether this index enforces uniqueness
    pub unique: bool,

    /// Whether this is the primary key index
    pub primary_key: bool,
}

/// Index field definition
#[derive(Debug, Clone)]
pub struct IndexField {
    /// Index of the field within the model
    pub field: usize,

    /// Scope of the index field
    pub scope: db::IndexScope,
}

impl Model {
    /// Get a field by index
    pub fn field(&self, index: usize) -> &Field {
        &self.fields[index]
    }

    /// Find a field by name
    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Iterate over primary key fields
    pub fn primary_key_fields(&self) -> impl Iterator<Item = &Field> {
        self.primary_key
            .fields
            .iter()
            .map(|&index| &self.fields[index])
    }
}

impl PrimaryKey {
    pub fn new(fields: Vec<usize>) -> Self {
        Self { fields }
    }
}

impl Index {
    pub fn new(fields: Vec<IndexField>, unique: bool, primary_key: bool) -> Self {
        Self {
            fields,
            unique,
            primary_key,
        }
    }
}

impl IndexField {
    pub fn new(field: usize, scope: db::IndexScope) -> Self {
        Self { field, scope }
    }
}
