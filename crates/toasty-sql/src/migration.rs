use std::borrow::Cow;

use toasty_core::{
    driver::{Capability, SqlPlaceholder},
    schema::{
        db::{Column, Schema, Table, Type, TypeEnum},
        diff,
    },
};

use crate::stmt::{AlterColumnChanges, AlterTable, AlterTableAction, DropTable, Name, Statement};

/// Returns `true` if the only difference between two columns is the variant
/// list of a named enum type. These changes are handled by `diff::Type`
/// (`ALTER TYPE ... ADD VALUE`) and should not produce column-level DDL.
fn is_named_enum_variant_only_change(previous: &Column, next: &Column) -> bool {
    if previous.name != next.name
        || previous.nullable != next.nullable
        || previous.comment != next.comment
        || previous.primary_key != next.primary_key
        || previous.auto_increment != next.auto_increment
    {
        return false;
    }

    matches!(
        (&previous.storage_ty, &next.storage_ty),
        (
            Type::Enum(TypeEnum { name: Some(a), .. }),
            Type::Enum(TypeEnum { name: Some(b), .. }),
        ) if a == b
    )
}

fn uses_postgresql_comments(capability: &Capability) -> bool {
    matches!(
        capability.sql_placeholder,
        Some(SqlPlaceholder::DollarNumber)
    )
}

fn uses_mysql_comments(capability: &Capability) -> bool {
    matches!(
        capability.sql_placeholder,
        Some(SqlPlaceholder::QuestionMark)
    )
}

fn emit_postgresql_comments<'a>(
    result: &mut Vec<MigrationStatement<'a>>,
    table: &Table,
    schema: Cow<'a, Schema>,
    capability: &Capability,
) {
    if !uses_postgresql_comments(capability) {
        return;
    }

    if table.comment.is_some() {
        result.push(MigrationStatement::new(
            Statement::comment_on_table(table),
            schema.clone(),
        ));
    }

    for column in &table.columns {
        if column.comment.is_some() {
            result.push(MigrationStatement::new(
                Statement::comment_on_column(table, column),
                schema.clone(),
            ));
        }
    }
}

/// A migration step pairing a DDL [`Statement`] with the [`Schema`] it applies against.
///
/// Each `MigrationStatement` carries a snapshot of the schema at the point where
/// the statement should be serialized. This is necessary because rename and
/// recreation operations modify the schema as they go.
pub struct MigrationStatement<'a> {
    statement: Statement,
    schema: Cow<'a, Schema>,
}

impl<'a> MigrationStatement<'a> {
    fn new(statement: Statement, schema: Cow<'a, Schema>) -> Self {
        MigrationStatement { statement, schema }
    }

    /// Generates migration statements from a [`diff::Schema`].
    ///
    /// Walks the diff's type, table, column, and index changes and produces
    /// the corresponding DDL statements. Type changes (CREATE TYPE, ALTER
    /// TYPE) are emitted before table changes. On databases that lack
    /// `ALTER COLUMN` support (e.g. SQLite), column type changes trigger a
    /// full table recreation sequence.
    pub fn from_diff(schema_diff: &'a diff::Schema<'a>, capability: &Capability) -> Vec<Self> {
        let mut result = Vec::new();

        // Emit enum type changes before table changes (tables may reference
        // newly created types).
        if capability.named_enum_types {
            let types_diff = schema_diff.types();
            for item in types_diff.iter() {
                match item {
                    diff::Type::Create(ty) => {
                        result.push(Self::new(
                            Statement::create_enum_type(ty),
                            Cow::Borrowed(schema_diff.next()),
                        ));
                    }
                    diff::Type::AddVariants { ty, added } => {
                        let type_name = ty.name.as_deref().expect("named enum type");
                        for variant in added {
                            result.push(Self::new(
                                Statement::alter_type_add_value(type_name, variant),
                                Cow::Borrowed(schema_diff.next()),
                            ));
                        }
                    }
                }
            }
        }

        for table in schema_diff.tables().iter() {
            match table {
                diff::Table::Create(table) => {
                    result.push(Self::new(
                        Statement::create_table(table, capability),
                        Cow::Borrowed(schema_diff.next()),
                    ));
                    emit_postgresql_comments(
                        &mut result,
                        table,
                        Cow::Borrowed(schema_diff.next()),
                        capability,
                    );
                    for index in &table.indices {
                        if index.primary_key {
                            continue; // PK indices are created as part of CREATE TABLE
                        }
                        result.push(Self::new(
                            Statement::create_index(index),
                            Cow::Borrowed(schema_diff.next()),
                        ));
                    }
                }
                diff::Table::Drop(table) => result.push(Self::new(
                    Statement::drop_table(table),
                    Cow::Borrowed(schema_diff.previous()),
                )),
                diff::Table::Alter {
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

                    if previous.comment != next.comment {
                        if uses_postgresql_comments(capability) {
                            result.push(Self::new(
                                Statement::comment_on_table(next),
                                Cow::Borrowed(schema_diff.next()),
                            ));
                        } else if uses_mysql_comments(capability) {
                            result.push(Self::new(
                                Statement::alter_table_comment(next),
                                Cow::Borrowed(schema_diff.next()),
                            ));
                        }
                    }

                    // Check if any column alteration requires table recreation
                    // (e.g. SQLite can't alter column type/nullability/auto_increment)
                    let needs_recreation = !capability.schema_mutations.alter_column_type
                        && columns.iter().any(|item| {
                            matches!(
                                item,
                                diff::Column::Alter {
                                    previous: prev_col,
                                    next: next_col
                                } if AlterColumnChanges::from_diff(prev_col, next_col).has_type_change()
                                    && !(capability.named_enum_types
                                        && is_named_enum_variant_only_change(prev_col, next_col))
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
                            diff::Index::Create(index) => {
                                result.push(Self::new(
                                    Statement::create_index(index),
                                    Cow::Borrowed(schema_diff.next()),
                                ));
                            }
                            diff::Index::Drop(index) => {
                                result.push(Self::new(
                                    Statement::drop_index(index),
                                    Cow::Borrowed(schema_diff.previous()),
                                ));
                            }
                            diff::Index::Alter { previous, next } => {
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
        columns: &[diff::Column<'_>],
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
                    .any(|item| matches!(item, diff::Column::Add(c) if c.id == col.id))
            })
            .map(|col| {
                let target_name = Name::from(&col.name[..]);
                // Check if this column was renamed
                let source_name = columns
                    .iter()
                    .find_map(|item| match item {
                        diff::Column::Alter {
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
        columns: &[diff::Column<'_>],
        capability: &Capability,
    ) {
        for item in columns.iter() {
            match item {
                diff::Column::Add(column) => {
                    result.push(Self::new(
                        Statement::add_column(column, capability),
                        schema.clone(),
                    ));
                    if column.comment.is_some() && uses_postgresql_comments(capability) {
                        let table = schema.table(column.id.table);
                        result.push(Self::new(
                            Statement::comment_on_column(table, column),
                            schema.clone(),
                        ));
                    }
                }
                diff::Column::Drop(column) => {
                    result.push(Self::new(Statement::drop_column(column), schema.clone()));
                }
                diff::Column::Alter {
                    previous,
                    next: col_next,
                } => {
                    // Skip column-level DDL for named enum variant changes — those
                    // are handled by diff::Type (ALTER TYPE ... ADD VALUE).
                    if capability.named_enum_types
                        && is_named_enum_variant_only_change(previous, col_next)
                    {
                        continue;
                    }

                    if previous.comment != col_next.comment && uses_postgresql_comments(capability)
                    {
                        let table = schema.table(col_next.id.table);
                        result.push(Self::new(
                            Statement::comment_on_column(table, col_next),
                            schema.clone(),
                        ));
                    }

                    let mut changes = AlterColumnChanges::from_diff(previous, col_next);
                    if !uses_mysql_comments(capability) {
                        changes.new_comment = None;
                    }
                    let changes = if capability.schema_mutations.alter_column_properties_atomic {
                        vec![changes]
                    } else {
                        changes.split()
                    };

                    for changes in changes {
                        if changes.is_empty() {
                            continue;
                        }
                        result.push(Self::new(
                            Statement::alter_column(previous, changes, capability),
                            schema.clone(),
                        ));
                    }
                }
            }
        }
    }

    /// Returns the DDL statement for this migration step.
    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    /// Returns the schema snapshot this statement should be serialized against.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}
