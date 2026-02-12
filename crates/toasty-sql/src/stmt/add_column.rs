use super::{ColumnDef, Statement};

use toasty_core::{
    driver::Capability,
    schema::db::{Column, TableId},
};

/// A statement to add a column to a table.
#[derive(Debug, Clone)]
pub struct AddColumn {
    /// ID of the table to add the column to.
    pub table: TableId,

    /// Column definition.
    pub column: ColumnDef,
}

impl Statement {
    /// Adds a column to a table.
    pub fn add_column(column: &Column, capability: &Capability) -> Self {
        AddColumn {
            table: column.id.table,
            column: ColumnDef::from_schema(column, &capability.storage_types),
        }
        .into()
    }
}

impl From<AddColumn> for Statement {
    fn from(value: AddColumn) -> Self {
        Self::AddColumn(value)
    }
}
