use toasty_core::{
    driver,
    schema::db::{self, Column},
};

/// A column definition used in `CREATE TABLE` and `ADD COLUMN` statements.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column name.
    pub name: String,
    /// Storage type (e.g. `INTEGER`, `TEXT`).
    pub ty: db::Type,
    /// When `true`, the column has a `NOT NULL` constraint.
    pub not_null: bool,
    /// When `true`, the column auto-increments.
    pub auto_increment: bool,
}

impl ColumnDef {
    pub(crate) fn from_schema(column: &Column, _storage_types: &driver::StorageTypes) -> Self {
        Self {
            name: column.name.clone(),
            ty: column.storage_ty.clone(),
            not_null: !column.nullable,
            auto_increment: column.auto_increment,
        }
    }
}
