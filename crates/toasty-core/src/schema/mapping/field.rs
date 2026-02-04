use crate::schema::db::ColumnId;

/// Maps a model field to its database storage representation.
///
/// Different field types have different storage strategies:
/// - Primitive fields map to a single column
/// - Embedded fields flatten to multiple columns (one per primitive field in the embedded struct)
/// - Relation fields don't map directly to columns and are represented as `None`
#[derive(Debug, Clone)]
pub enum Field {
    /// A primitive field stored in a single column.
    Primitive(FieldPrimitive),

    /// An embedded struct field flattened into multiple columns.
    Embedded(FieldEmbedded),
}

impl Field {
    pub fn as_primitive(&self) -> Option<&FieldPrimitive> {
        match self {
            Field::Primitive(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_primitive_mut(&mut self) -> Option<&mut FieldPrimitive> {
        match self {
            Field::Primitive(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_embedded(&self) -> Option<&FieldEmbedded> {
        match self {
            Field::Embedded(e) => Some(e),
            _ => None,
        }
    }
}

/// Maps a primitive field to its table column.
#[derive(Debug, Clone)]
pub struct FieldPrimitive {
    /// The table column that stores this field's value.
    pub column: ColumnId,

    /// Index into `Model::model_to_table` for this field's lowering expression.
    ///
    /// The expression at this index converts the model field value to the
    /// column value during `INSERT` and `UPDATE` operations.
    pub lowering: usize,
}

/// Maps an embedded struct field to its flattened column representation.
///
/// Embedded fields are stored by flattening their primitive fields into columns
/// with names like `{field}_{embedded_field}`. This structure tracks the mapping
/// for each field in the embedded struct.
#[derive(Debug, Clone)]
pub struct FieldEmbedded {
    /// Per-field mappings for the embedded struct's fields.
    ///
    /// Indexed by field index within the embedded model. Contains `None` for
    /// relation fields (which aren't allowed in embedded types, but we handle
    /// gracefully) and nested embedded fields (not yet implemented).
    pub fields: Vec<Option<Field>>,
}
