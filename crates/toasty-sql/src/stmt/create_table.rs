use super::*;

use toasty_core::{schema::db::Table, stmt};

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
    pub fn create_table(table: &Table) -> Statement {
        CreateTable {
            name: Name::from(&table.name[..]),
            columns: table.columns.iter().map(ColumnDef::from_schema).collect(),
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
        Statement::CreateTable(value)
    }
}
