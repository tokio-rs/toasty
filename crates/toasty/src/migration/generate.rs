use crate::{
    db::Driver,
    schema::{db, diff},
};

use super::Snapshot;

/// A generated database migration and the schema snapshot it advances to.
///
/// This is the reusable core of migration generation. It deliberately does not
/// include filenames, history IDs, or persistence decisions; callers own those
/// policies.
#[derive(Debug)]
pub struct Generated {
    /// The driver-specific migration statements.
    pub migration: db::Migration,

    /// Snapshot of the schema after the migration is applied.
    pub snapshot: Snapshot,
}

/// Options controlling how a migration schema is materialized.
#[derive(Debug, Clone, Copy, Default)]
pub struct GenerateOptions {
    /// Whether declared table and column comments are managed by migrations.
    pub schema_comments: bool,
}

impl GenerateOptions {
    /// Creates migration-generation options with comments disabled.
    pub const fn new() -> Self {
        Self {
            schema_comments: false,
        }
    }

    /// Enables or disables schema comment management.
    pub const fn schema_comments(mut self, enabled: bool) -> Self {
        self.schema_comments = enabled;
        self
    }
}

/// Generate a database migration from `previous` to `next`.
///
/// Returns `None` when the schemas are equivalent after applying
/// `rename_hints`.
pub fn generate(
    driver: &dyn Driver,
    previous: &db::Schema,
    next: &db::Schema,
    rename_hints: &diff::RenameHints,
) -> Option<Generated> {
    generate_with_options(
        driver,
        previous,
        next,
        rename_hints,
        GenerateOptions::default(),
    )
}

/// Generate a database migration using the supplied materialization options.
pub fn generate_with_options(
    driver: &dyn Driver,
    previous: &db::Schema,
    next: &db::Schema,
    rename_hints: &diff::RenameHints,
    options: GenerateOptions,
) -> Option<Generated> {
    let capability = driver.capability();
    let mut effective_previous = previous.clone();
    let mut effective_next = next.clone();

    if !options.schema_comments {
        carry_comments(&effective_previous, &mut effective_next, rename_hints);
    }

    filter_unsupported_comments(&mut effective_previous, capability);
    filter_unsupported_comments(&mut effective_next, capability);

    let schema_diff = diff::Schema::from(&effective_previous, &effective_next, rename_hints);

    if schema_diff.is_empty() {
        return None;
    }

    Some(Generated {
        migration: driver.generate_migration(&schema_diff),
        snapshot: Snapshot::new(effective_next),
    })
}

fn carry_comments(previous: &db::Schema, next: &mut db::Schema, rename_hints: &diff::RenameHints) {
    for table in &mut next.tables {
        table.comment = None;
        for column in &mut table.columns {
            column.comment = None;
        }
    }

    for previous_table in &previous.tables {
        let next_table_index = rename_hints
            .get_table(previous_table.id)
            .map(|id| id.0)
            .or_else(|| {
                next.tables
                    .iter()
                    .position(|table| table.name == previous_table.name)
            });
        let Some(next_table_index) = next_table_index else {
            continue;
        };
        let next_table = &mut next.tables[next_table_index];
        next_table.comment.clone_from(&previous_table.comment);

        for previous_column in &previous_table.columns {
            let next_column_index = rename_hints
                .get_column(previous_column.id)
                .filter(|id| id.table == next_table.id)
                .map(|id| id.index)
                .or_else(|| {
                    next_table
                        .columns
                        .iter()
                        .position(|column| column.name == previous_column.name)
                });
            if let Some(next_column_index) = next_column_index {
                next_table.columns[next_column_index]
                    .comment
                    .clone_from(&previous_column.comment);
            }
        }
    }
}

fn filter_unsupported_comments(schema: &mut db::Schema, capability: &crate::db::Capability) {
    for table in &mut schema.tables {
        if !capability.schema_comments.table {
            table.comment = None;
        }
        if !capability.schema_comments.column {
            for column in &mut table.columns {
                column.comment = None;
            }
        }
    }
}
