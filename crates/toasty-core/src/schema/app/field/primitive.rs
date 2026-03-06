use crate::{schema::db, stmt};

/// The serialization format used to store a field value.
#[derive(Debug, Clone)]
pub enum SerializeFormat {
    /// Serialize as JSON using serde_json.
    Json,
}

#[derive(Debug, Clone)]
pub struct FieldPrimitive {
    /// The field's primitive type
    pub ty: stmt::Type,

    /// The database storage type of the field.
    ///
    /// This is specified as a hint.
    pub storage_ty: Option<db::Type>,

    /// If set, the field value is serialized using the specified format
    /// before being stored in the database.
    pub serialize: Option<SerializeFormat>,
}
