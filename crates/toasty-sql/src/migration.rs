use std::borrow::Cow;

use toasty_core::{
    driver::Capability,
    schema::db::{
        ColumnsDiff, ColumnsDiffItem, IndicesDiffItem, Schema, SchemaDiff, Table, TablesDiffItem,
    },
};

use crate::stmt::{AlterColumnChanges, AlterTable, AlterTableAction, DropTable, Name, Statement};

pub struct MigrationStatement<'a> {
    statement: Statement,
    schema: Cow<'a, Schema>,
}

impl<'a> MigrationStatement<'a> {
    fn new(statement: Statement, schema: Cow<'a, Schema>) -> Self {
        MigrationStatement { statement, schema }
    }

    pub fn from_diff(schema_diff: &'a SchemaDiff<'a>, capability: &Capability) -> Vec<Self> {
        let mut result = Vec::new();
        for table in schema_diff.tables().iter() {
            match table {
                TablesDiffItem::CreateTable(table) => {
                    result.push(Self::new(
                        Statement::create_table(table, capability),
                        Cow::Borrowed(schema_diff.next()),
                    ));
                    for index in &table.indices {
                        result.push(Self::new(
                            Statement::create_index(index),
                            Cow::Borrowed(schema_diff.next()),
                        ));
                    }
                }
                TablesDiffItem::DropTable(table) => result.push(Self::new(
                    Statement::drop_table(table),
                    Cow::Borrowed(schema_diff.previous()),
                )),
                TablesDiffItem::AlterTable {
                    previous,
                    next,
                    columns,
                    indices,
                    ..
                } => {
                    let mut schema = Cow::Borrowed(schema_diff.previous());
                    if previous.name != next.name {
                        result.push(Self::new(
                            Statement::alter_table_rename_to(previous, &next.name),
                            schema.clone(),
                        ));
                        schema.to_mut().table_mut(previous.id).name = next.name.clone();
                    }

                    // Check if any column alteration requires table recreation
                    // (e.g. SQLite can't alter column type/nullability/auto_increment)
                    let needs_recreation = !capability.schema_mutations.alter_column_type
                        && columns.iter().any(|item| {
                            matches!(
                                item,
                                ColumnsDiffItem::AlterColumn {
                                    previous: prev_col,
                                    next: next_col
                                } if AlterColumnChanges::from_diff(prev_col, next_col).has_type_change()
                            )
                        });

                    if needs_recreation {
                        Self::emit_table_recreation(
                            &mut result,
                            schema,
                            previous,
                            next,
                            columns,
                            capability,
                        );
                    } else {
                        Self::emit_column_changes(&mut result, schema, columns, capability);
                    }

                    // Indices diff
                    for item in indices.iter() {
                        match item {
                            IndicesDiffItem::CreateIndex(index) => {
                                result.push(Self::new(
                                    Statement::create_index(index),
                                    Cow::Borrowed(schema_diff.next()),
                                ));
                            }
                            IndicesDiffItem::DropIndex(index) => {
                                result.push(Self::new(
                                    Statement::drop_index(index),
                                    Cow::Borrowed(schema_diff.previous()),
                                ));
                            }
                            IndicesDiffItem::AlterIndex { previous, next } => {
                                result.push(Self::new(
                                    Statement::drop_index(previous),
                                    Cow::Borrowed(schema_diff.previous()),
                                ));
                                result.push(Self::new(
                                    Statement::create_index(next),
                                    Cow::Borrowed(schema_diff.next()),
                                ));
                            }
                        }
                    }
                }
            }
        }
        result
    }

    fn emit_table_recreation(
        result: &mut Vec<Self>,
        schema: Cow<'a, Schema>,
        previous: &Table,
        next: &Table,
        columns: &ColumnsDiff<'_>,
        capability: &Capability,
    ) {
        let current_name = schema.table(previous.id).name.clone();
        let temp_name = format!("_toasty_new_{}", current_name);

        // 1. PRAGMA foreign_keys = OFF
        result.push(Self::new(
            Statement::pragma_disable_foreign_keys(),
            schema.clone(),
        ));

        // 2. CREATE TABLE temp with new schema
        let temp_schema = {
            let mut s = schema.as_ref().clone();
            let t = s.table_mut(next.id);
            t.name = temp_name.clone();
            t.columns = next.columns.clone();
            t.primary_key = next.primary_key.clone();
            s
        };
        result.push(Self::new(
            Statement::create_table(next, capability),
            Cow::Owned(temp_schema),
        ));

        // 3. INSERT INTO temp SELECT ... FROM current
        let column_mappings: Vec<(Name, Name)> = next
            .columns
            .iter()
            .filter(|col| {
                // Skip added columns (no source data)
                !columns
                    .iter()
                    .any(|item| matches!(item, ColumnsDiffItem::AddColumn(c) if c.id == col.id))
            })
            .map(|col| {
                let target_name = Name::from(&col.name[..]);
                // Check if this column was renamed
                let source_name = columns
                    .iter()
                    .find_map(|item| match item {
                        ColumnsDiffItem::AlterColumn {
                            previous: prev_col,
                            next: next_col,
                        } if next_col.id == col.id && prev_col.name != next_col.name => {
                            Some(Name::from(&prev_col.name[..]))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| Name::from(&col.name[..]));
                (target_name, source_name)
            })
            .collect();

        result.push(Self::new(
            Statement::copy_table(
                Name::from(current_name.as_str()),
                Name::from(temp_name.as_str()),
                column_mappings,
            ),
            schema.clone(),
        ));

        // 4. DROP TABLE current
        result.push(Self::new(
            DropTable {
                name: Name::from(current_name.as_str()),
                if_exists: false,
            }
            .into(),
            schema.clone(),
        ));

        // 5. ALTER TABLE temp RENAME TO current
        result.push(Self::new(
            AlterTable {
                name: Name::from(temp_name.as_str()),
                action: AlterTableAction::RenameTo(Name::from(current_name.as_str())),
            }
            .into(),
            schema.clone(),
        ));

        // 6. PRAGMA foreign_keys = ON
        result.push(Self::new(
            Statement::pragma_enable_foreign_keys(),
            schema.clone(),
        ));
    }

    fn emit_column_changes(
        result: &mut Vec<Self>,
        schema: Cow<'a, Schema>,
        columns: &ColumnsDiff<'_>,
        capability: &Capability,
    ) {
        for item in columns.iter() {
            match item {
                ColumnsDiffItem::AddColumn(column) => {
                    result.push(Self::new(
                        Statement::add_column(column, capability),
                        schema.clone(),
                    ));
                }
                ColumnsDiffItem::DropColumn(column) => {
                    result.push(Self::new(Statement::drop_column(column), schema.clone()));
                }
                ColumnsDiffItem::AlterColumn {
                    previous,
                    next: col_next,
                } => {
                    let changes = AlterColumnChanges::from_diff(previous, col_next);
                    let changes = if capability.schema_mutations.alter_column_properties_atomic {
                        vec![changes]
                    } else {
                        changes.split()
                    };

                    for changes in changes {
                        result.push(Self::new(
                            Statement::alter_column(previous, changes, capability),
                            schema.clone(),
                        ));
                    }
                }
            }
        }
    }

    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}
