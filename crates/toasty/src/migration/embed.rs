use std::collections::HashSet;

use crate::{Db, Result, schema::db::Migration as DbMigration};

/// A migration SQL file and its tracking metadata.
#[derive(Debug, Clone, Copy)]
pub struct MigrationFile {
    id: u64,
    name: &'static str,
    sql: &'static str,
}

impl MigrationFile {
    /// Creates a migration file.
    pub const fn new(id: u64, name: &'static str, sql: &'static str) -> Self {
        Self { id, name, sql }
    }

    /// Returns the migration ID recorded in the database.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the migration file name.
    pub fn name(&self) -> &str {
        self.name
    }

    /// Returns the migration SQL.
    pub fn sql(&self) -> &str {
        self.sql
    }
}

/// An ordered collection of migrations that can be applied to a database.
#[derive(Debug, Clone, Copy, Default)]
pub struct MigrationSet {
    migrations: &'static [MigrationFile],
}

impl MigrationSet {
    /// Creates a migration set in application order.
    pub const fn new(migrations: &'static [MigrationFile]) -> Self {
        Self { migrations }
    }

    /// Returns the migrations in application order.
    pub fn migrations(&self) -> &[MigrationFile] {
        self.migrations
    }

    /// Applies pending migrations to `db` in order.
    ///
    /// Migration IDs already recorded by the driver are skipped. If a
    /// migration fails, later migrations are not attempted.
    pub async fn apply(&self, db: &Db) -> Result<MigrationReport> {
        let conn = db.connection().await?;
        let mut applied_ids = conn
            .applied_migrations()
            .await?
            .into_iter()
            .map(|migration| migration.id())
            .collect::<HashSet<_>>();
        let mut report = MigrationReport::default();

        for migration in self.migrations {
            if applied_ids.contains(&migration.id) {
                report.skipped += 1;
                continue;
            }

            conn.apply_migration(
                migration.id,
                migration.name,
                DbMigration::new_sql(migration.sql.to_string()),
            )
            .await?;
            applied_ids.insert(migration.id);
            report.applied += 1;
        }

        Ok(report)
    }
}

/// Counts returned after applying a [`MigrationSet`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MigrationReport {
    applied: usize,
    skipped: usize,
}

impl MigrationReport {
    /// Returns the number of migrations applied by this call.
    pub fn applied(&self) -> usize {
        self.applied
    }

    /// Returns the number of migrations skipped because their IDs were already applied.
    pub fn skipped(&self) -> usize {
        self.skipped
    }
}
