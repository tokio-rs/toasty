use super::*;

use crate::schema::db::Table;

#[derive(Debug, Clone)]
pub struct CreateTable {
    /// Name of the table
    pub name: Name,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// Primary key clause
    pub primary_key: Option<Box<Expr>>,
}

impl Statement {
    pub fn create_table(table: &Table) -> Statement {
        CreateTable {
            name: Name::from(&table.name[..]),
            columns: table.columns.iter().map(ColumnDef::from_schema).collect(),
            primary_key: Some(Box::new(Expr::record(
                table
                    .primary_key
                    .columns
                    .iter()
                    .map(|col| Expr::column(*col)),
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
