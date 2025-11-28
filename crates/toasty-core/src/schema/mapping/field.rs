use crate::schema::db::ColumnId;

/// Maps a single primitive model field to its corresponding table column.
///
/// Only primitive fields have a `Field` mapping. Relation fields (`BelongsTo`,
/// `HasMany`, `HasOne`) do not map directly to columns and are represented as
/// `None` in the parent `Model::fields` vector.
#[derive(Debug, Clone)]
pub struct Field {
    /// The table column that stores this field's value.
    pub column: ColumnId,

    /// Index into `Model::model_to_table` for this field's lowering expression.
    ///
    /// The expression at this index converts the model field value to the
    /// column value during `INSERT` and `UPDATE` operations.
    pub lowering: usize,
}
