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
    let schema_diff = diff::Schema::from(previous, next, rename_hints);

    if schema_diff.is_empty() {
        return None;
    }

    Some(Generated {
        migration: driver.generate_migration(&schema_diff),
        snapshot: Snapshot::new(next.clone()),
    })
}
