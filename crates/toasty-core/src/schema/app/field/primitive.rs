use crate::{schema::db, stmt};

/// The serialization format used to store a field value in the database.
///
/// When a field's in-memory type does not map directly to a database column
/// type, the value is serialized into a format the database can store (e.g.,
/// a JSON string column).
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::SerializeFormat;
///
/// let fmt = SerializeFormat::Json;
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SerializeFormat {
    /// Serialize the value as JSON using `serde_json`.
    Json,
}

/// A primitive (non-relation, non-embedded) field type.
///
/// Primitive fields map directly to a single database column. They carry the
/// application-level type, an optional storage-type hint for the database
/// driver, and an optional serialization format.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::app::FieldPrimitive;
/// use toasty_core::stmt::Type;
///
/// let prim = FieldPrimitive {
///     ty: Type::String,
///     storage_ty: None,
///     serialize: None,
/// };
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldPrimitive {
    /// The application-level primitive type of this field.
    pub ty: stmt::Type,

    /// Optional database storage type hint. When set, the driver uses this
    /// type instead of inferring one from `ty`.
    pub storage_ty: Option<db::Type>,

    /// If set, the field value is serialized using the specified format
    /// before being written to the database.
    pub serialize: Option<SerializeFormat>,
}
