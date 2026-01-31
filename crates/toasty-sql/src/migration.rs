use toasty_core::{
    driver::Capability,
    schema::db::{ColumnsDiffItem, IndicesDiffItem, Schema, SchemaDiff, TablesDiffItem},
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
                TablesDiffItem::AlterTable {
                    from,
                    to,
                    columns,
                    indices,
                } => {
                    // Columns diff
                    for item in columns.iter() {
                        match item {
                            ColumnsDiffItem::AddColumn(column) => {
                                result.push(MigrationStatement {
                                    statement: Statement::add_column(column, capability),
                                    schema: schema_diff.next(),
                                });
                            }
                            ColumnsDiffItem::DropColumn(column) => {
                                result.push(MigrationStatement {
                                    statement: Statement::drop_column(column),
                                    schema: schema_diff.previous(),
                                });
                            }
                            ColumnsDiffItem::AlterColumn { from, to } => {
                                result.push(MigrationStatement {
                                    statement: Statement::drop_column(from),
                                    schema: schema_diff.previous(),
                                });
                                result.push(MigrationStatement {
                                    statement: Statement::add_column(to, capability),
                                    schema: schema_diff.next(),
                                });
                            }
                        }
                    }

                    // Indices diff
                    for item in indices.iter() {
                        match item {
                            IndicesDiffItem::CreateIndex(index) => {
                                result.push(MigrationStatement {
                                    statement: Statement::create_index(index),
                                    schema: schema_diff.next(),
                                });
                            }
                            IndicesDiffItem::DropIndex(index) => {
                                result.push(MigrationStatement {
                                    statement: Statement::drop_index(index),
                                    schema: schema_diff.previous(),
                                });
                            }
                            IndicesDiffItem::AlterIndex { from, to } => {
                                result.push(MigrationStatement {
                                    statement: Statement::drop_index(from),
                                    schema: schema_diff.previous(),
                                });
                                result.push(MigrationStatement {
                                    statement: Statement::create_index(to),
                                    schema: schema_diff.next(),
                                });
                            }
                        }
                    }
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
