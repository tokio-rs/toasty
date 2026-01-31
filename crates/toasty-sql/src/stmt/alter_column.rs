use crate::stmt::ColumnDef;

use super::Statement;

use toasty_core::{
    driver::Capability,
    schema::db::{Column, ColumnId, Type},
};

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumn {
    /// ID of the column being altered.
    pub id: ColumnId,

    /// Current column definition.
    pub column_def: ColumnDef,

    /// Changes to be made to the column.
    pub changes: AlterColumnChanges,
}

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumnChanges {
    /// New name for the column (if renaming).
    pub new_name: Option<String>,

    /// New type information.
    pub new_ty: Option<Type>,

    /// New nullability constraint.
    pub new_not_null: Option<bool>,

    /// New auto increment behavior.
    pub new_auto_increment: Option<bool>,
}

impl Statement {
    /// Alters a column.
    pub fn alter_column(
        column: &Column,
        changes: AlterColumnChanges,
        capability: &Capability,
    ) -> Self {
        AlterColumn {
            id: column.id,
            column_def: ColumnDef::from_schema(column, &capability.storage_types),
            changes,
        }
        .into()
    }
}

impl From<AlterColumn> for Statement {
    fn from(value: AlterColumn) -> Self {
        Self::AlterColumn(value)
    }
}
