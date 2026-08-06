use crate::schema::{db, diff};

use super::Snapshot;

pub use toasty_sql::Flavor;

/// A generated database migration and the schema snapshot it advances to.
///
/// This is the reusable core of migration generation. It deliberately does not
/// include filenames, history IDs, or persistence decisions; callers own those
/// policies.
#[derive(Debug)]
pub struct Generated {
    /// The migration statements, in the target flavor's dialect.
    pub migration: db::Migration,

    /// Snapshot of the schema after the migration is applied.
    pub snapshot: Snapshot,
}

/// Generate a database migration from `previous` to `next`, targeting
/// `flavor`.
///
/// Returns `None` when the schemas are equivalent after applying
/// `rename_hints`.
///
/// The migration depends on the SQL dialect, not on a connection, so this can
/// generate for a database that is not running — which is how
/// `toasty migrate generate --flavor <flavor>` works.
pub fn generate(
    flavor: Flavor,
    previous: &db::Schema,
    next: &db::Schema,
    rename_hints: &diff::RenameHints,
) -> Option<Generated> {
    let schema_diff = diff::Schema::from(previous, next, rename_hints);

    if schema_diff.is_empty() {
        return None;
    }

    Some(Generated {
        migration: toasty_sql::generate_migration(flavor, &schema_diff),
        snapshot: Snapshot::new(next.clone()),
    })
}
