use super::*;

use crate::schema::Table;

#[derive(Debug, Clone)]
pub struct CreateTable<'stmt> {
    /// Name of the table
    pub name: Name,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// Primary key clause
    pub primary_key: Option<Box<Expr<'stmt>>>,
}

impl<'stmt> Statement<'stmt> {
    pub fn create_table(table: &Table) -> Statement<'stmt> {
        CreateTable {
            name: Name::from(&table.name[..]),
            columns: table.columns.iter().map(ColumnDef::from_schema).collect(),
            primary_key: Some(Box::new(Expr::record(
                table.primary_key.columns.iter().map(Expr::column),
            ))),
        }
        .into()
    }
}

impl<'stmt> From<CreateTable<'stmt>> for Statement<'stmt> {
    fn from(value: CreateTable<'stmt>) -> Self {
        Statement::CreateTable(value)
    }
}
