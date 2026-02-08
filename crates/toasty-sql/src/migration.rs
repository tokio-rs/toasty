use toasty_core::{
    driver::Capability,
    schema::db::{ColumnsDiffItem, IndicesDiffItem, Schema, SchemaDiff, TablesDiffItem},
};

use crate::stmt::{AlterColumnChanges, Statement};

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
                    columns, indices, ..
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
                            ColumnsDiffItem::AlterColumn { previous, next } => {
                                let changes = AlterColumnChanges::from_diff(previous, next);
                                if !capability.schema_mutations.alter_column_type
                                    && changes.has_type_change()
                                {
                                    todo!();
                                } else {
                                    // Split up changes into multiple statements for databases that
                                    // do not changing multiple column properties in one statement.
                                    let changes = if capability
                                        .schema_mutations
                                        .alter_column_properties_atomic
                                    {
                                        vec![changes]
                                    } else {
                                        changes.split()
                                    };

                                    for changes in changes {
                                        result.push(MigrationStatement {
                                            statement: Statement::alter_column(
                                                previous, changes, capability,
                                            ),
                                            schema: schema_diff.previous(),
                                        });
                                    }
                                }
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
                            IndicesDiffItem::AlterIndex { previous, next } => {
                                result.push(MigrationStatement {
                                    statement: Statement::drop_index(previous),
                                    schema: schema_diff.previous(),
                                });
                                result.push(MigrationStatement {
                                    statement: Statement::create_index(next),
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
