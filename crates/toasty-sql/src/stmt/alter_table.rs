use super::{Name, Statement};

use toasty_core::schema::db::Table;

/// A statement to alter a SQL table.
#[derive(Debug, Clone)]
pub struct AlterTable {
    /// Current name of the table.
    pub name: Name,

    /// The alteration to apply.
    pub action: AlterTableAction,
}

/// The action to perform in an ALTER TABLE statement.
#[derive(Debug, Clone)]
pub enum AlterTableAction {
    /// Rename the table to a new name.
    RenameTo(Name),
}

impl Statement {
    /// Renames a table.
    pub fn alter_table_rename_to(table: &Table, new_name: &str) -> Self {
        AlterTable {
            name: Name::from(&table.name[..]),
            action: AlterTableAction::RenameTo(Name::from(new_name)),
        }
        .into()
    }
}

impl From<AlterTable> for Statement {
    fn from(value: AlterTable) -> Self {
        Self::AlterTable(value)
    }
}
