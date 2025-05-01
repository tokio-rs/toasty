use super::*;

use toasty_core::schema::db::Table;

/// A statement to drop a SQL table.
#[derive(Debug, Clone)]
pub struct DropTable {
    /// Name of the table.
    pub name: Name,

    /// Whether or not to add an `IF EXISTS` clause.
    pub if_exists: bool,
}

impl Statement {
    /// Drops a table.
    ///
    /// This function _does not_ add an `IF EXISTS` clause.
    pub fn drop_table(table: &Table) -> Self {
        DropTable {
            name: Name::from(&table.name[..]),
            if_exists: false,
        }
        .into()
    }

    /// Drops a table if it exists.
    ///
    /// This function _does_ add an `IF EXISTS` clause.
    pub fn drop_table_if_exists(table: &Table) -> Self {
        DropTable {
            name: Name::from(&table.name[..]),
            if_exists: true,
        }
        .into()
    }
}

impl From<DropTable> for Statement {
    fn from(value: DropTable) -> Self {
        Self::DropTable(value)
    }
}
