use super::*;

use crate::schema::db::Table;

/// A statement to drop a SQL table.
#[derive(Debug, Clone)]
pub struct DropTable {
    /// Name of the table.
    pub name: Name,
}

impl Statement {
    pub fn drop_table(table: &Table) -> Statement {
        DropTable {
            name: Name::from(&table.name[..]),
        }
        .into()
    }
}

impl From<DropTable> for Statement {
    fn from(value: DropTable) -> Self {
        Statement::DropTable(value)
    }
}
