use super::{ColumnDef, Statement};

use toasty_core::{
    driver::Capability,
    schema::db::{Table, TableId},
    stmt,
};

/// A `CREATE TABLE` statement.
#[derive(Debug, Clone)]
pub struct CreateTable {
    /// Name of the table
    pub table: TableId,

    /// Column definitions
    pub columns: Vec<ColumnDef>,

    /// Primary key clause
    pub primary_key: Option<Box<stmt::Expr>>,
}

impl Statement {
    /// Creates a `CREATE TABLE` statement from a schema [`Table`].
    pub fn create_table(table: &Table, capability: &Capability) -> Self {
        CreateTable {
            table: table.id,
            columns: table
                .columns
                .iter()
                .map(|column| ColumnDef::from_schema(column, &capability.storage_types, capability))
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
