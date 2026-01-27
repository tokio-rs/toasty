use toasty_core::{
    driver::Capability,
    schema::db::{Schema, SchemaDiff, TablesDiffItem},
};

use crate::stmt::Statement;

pub struct MigrationStatement<'a> {
    statement: Statement,
    schema: &'a Schema,
}

impl<'a> MigrationStatement<'a> {
    pub fn from_diff(schema_diff: &'a SchemaDiff<'a>, capability: &Capability) -> Vec<Self> {
        let mut result = Vec::new();
        for table in schema_diff.tables().iter() {
            match table {
                TablesDiffItem::CreateTable(table) => {
                    result.push(MigrationStatement {
                        statement: Statement::create_table(table, capability),
                        schema: schema_diff.next(),
                    });
                    for index in &table.indices {
                        result.push(MigrationStatement {
                            statement: Statement::create_index(index),
                            schema: schema_diff.next(),
                        });
                    }
                }
                TablesDiffItem::DropTable(table) => result.push(MigrationStatement {
                    statement: Statement::drop_table(table),
                    schema: schema_diff.previous(),
                }),
                TablesDiffItem::AlterTable { from, to, .. } => {
                    result.push(MigrationStatement {
                        statement: Statement::drop_table(from),
                        schema: schema_diff.previous(),
                    });
                    result.push(MigrationStatement {
                        statement: Statement::create_table(to, capability),
                        schema: schema_diff.next(),
                    });
                }
            }
        }
        result
    }

    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    pub fn schema(&self) -> &'a Schema {
        self.schema
    }
}
