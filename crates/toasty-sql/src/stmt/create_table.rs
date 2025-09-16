use super::{ColumnDef, Name, Statement};

use toasty_core::{driver::Capability, schema::db::Table, stmt};

#[derive(Debug, Clone)]
pub struct CreateTable {
    /// Name of the table
    pub name: Name,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// Primary key clause
    pub primary_key: Option<Box<stmt::Expr>>,
}

impl Statement {
    pub fn create_table(table: &Table, capability: &Capability) -> Self {
        CreateTable {
            name: Name::from(&table.name[..]),
            columns: table
                .columns
                .iter()
                .map(|column| ColumnDef::from_schema(column, &capability.storage_types))
                .collect(),
            primary_key: None, // TODO: Fix primary key handling for alias-based columns
        }
        .into()
    }
}

impl From<CreateTable> for Statement {
    fn from(value: CreateTable) -> Self {
        Self::CreateTable(value)
    }
}
