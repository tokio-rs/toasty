use super::*;

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
            primary_key: Some(Box::new(stmt::Expr::record(
                table
                    .primary_key
                    .columns
                    .iter()
                    .map(|col| stmt::Expr::column(*col)),
            ))),
        }
        .into()
    }
}

impl From<CreateTable> for Statement {
    fn from(value: CreateTable) -> Self {
        Self::CreateTable(value)
    }
}
