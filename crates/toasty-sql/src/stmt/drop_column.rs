use super::{Name, Statement};

use toasty_core::schema::db::{Column, TableId};

/// A statement to drop a column from a table.
#[derive(Debug, Clone)]
pub struct DropColumn {
    /// ID of the table to drop the column from.
    pub table: TableId,

    /// Name of the column to drop.
    pub name: Name,

    /// Whether or not to add an `IF EXISTS` clause.
    pub if_exists: bool,
}

impl Statement {
    /// Drops a column from a table.
    ///
    /// This function _does not_ add an `IF EXISTS` clause.
    pub fn drop_column(column: &Column) -> Self {
        DropColumn {
            table: column.id.table,
            name: Name::from(&column.name[..]),
            if_exists: false,
        }
        .into()
    }

    /// Drops a column from a table if it exists.
    ///
    /// This function _does_ add an `IF EXISTS` clause.
    pub fn drop_column_if_exists(column: &Column) -> Self {
        DropColumn {
            table: column.id.table,
            name: Name::from(&column.name[..]),
            if_exists: true,
        }
        .into()
    }
}

impl From<DropColumn> for Statement {
    fn from(value: DropColumn) -> Self {
        Self::DropColumn(value)
    }
}
