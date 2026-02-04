use crate::schema::db::ColumnId;

/// Maps a model field to its database storage representation.
///
/// Different field types have different storage strategies:
/// - Primitive fields map to a single column
/// - Embedded fields flatten to multiple columns (one per primitive field in the embedded struct)
/// - Relation fields (`BelongsTo`, `HasMany`, `HasOne`) don't have direct column storage
#[derive(Debug, Clone)]
pub enum Field {
    /// A primitive field stored in a single column.
    Primitive(FieldPrimitive),

    /// An embedded struct field flattened into multiple columns.
    Embedded(FieldEmbedded),

    /// A relation field that doesn't map to columns in this table.
    ///
    /// Relations are resolved through joins or foreign keys in other tables,
    /// so they don't have column mappings in the source model.
    Relation,
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
    /// Indexed by field index within the embedded model. Relation fields use
    /// `Field::Relation` (though they aren't allowed in embedded types).
    pub fields: Vec<Field>,
}
