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
    pub fn create_table(table: &Table) -> Self {
        CreateTable {
            name: Name::from(&table.name[..]),
            columns: table
                .columns
                .iter()
                .map(|column| {
                    let indexed = table.indices.iter().any(|index| {
                        index
                            .columns
                            .iter()
                            .any(|index_column| index_column.column == column.id)
                    });

                    ColumnDef::from_schema(column, indexed)
                })
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
