use super::{Name, Statement};

use toasty_core::{
    driver::Capability,
    schema::db::{Column, TableId, Type},
};

/// Represents the new type and nullability for a column.
#[derive(Debug, Clone)]
pub struct ColumnTypeChange {
    /// The new storage type for the column.
    pub storage_ty: Type,
    /// Whether the column should be nullable.
    pub nullable: bool,
}

/// A statement to alter a column in a table.
#[derive(Debug, Clone)]
pub struct AlterColumn {
    /// ID of the table containing the column.
    pub table: TableId,

    /// Current name of the column.
    pub current_name: Name,

    /// New name for the column (if renaming).
    pub new_name: Option<Name>,

    /// New type information (if changing type or nullability).
    pub new_type: Option<ColumnTypeChange>,
}

impl Statement {
    /// Alters a column, detecting what changed between from and to.
    pub fn alter_column(from: &Column, to: &Column, capability: &Capability) -> Self {
        assert_eq!(from.id, to.id, "Cannot alter between different column IDs");

        // Check if name changed
        let new_name = if from.name != to.name {
            Some(Name::from(&to.name[..]))
        } else {
            None
        };

        // Check if type or nullable changed
        let new_type = if from.storage_ty != to.storage_ty || from.nullable != to.nullable {
            Some(ColumnTypeChange {
                storage_ty: to.storage_ty.clone(),
                nullable: to.nullable,
            })
        } else {
            None
        };

        AlterColumn {
            table: from.id.table,
            current_name: Name::from(&from.name[..]),
            new_name,
            new_type,
        }
        .into()
    }
}

impl From<AlterColumn> for Statement {
    fn from(value: AlterColumn) -> Self {
        Self::AlterColumn(value)
    }
}
