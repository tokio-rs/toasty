use std::{borrow::Cow, collections::HashSet};

use crate::{Db, Result, schema::db::Migration as DbMigration};

/// A migration SQL file and its tracking metadata.
#[derive(Debug, Clone)]
pub struct MigrationFile {
    id: u64,
    name: Cow<'static, str>,
    sql: Cow<'static, str>,
}

impl MigrationFile {
    /// Creates a migration file.
    pub fn new(
        id: u64,
        name: impl Into<Cow<'static, str>>,
        sql: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            sql: sql.into(),
        }
    }

    /// Returns the migration ID recorded in the database.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the migration file name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the migration SQL.
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

/// An ordered collection of migrations that can be applied to a database.
#[derive(Debug, Clone, Default)]
pub struct MigrationSet {
    migrations: Vec<MigrationFile>,
}

impl MigrationSet {
    /// Creates a migration set in application order.
    pub fn new(migrations: impl IntoIterator<Item = MigrationFile>) -> Self {
        Self {
            migrations: migrations.into_iter().collect(),
        }
    }

    /// Returns the migrations in application order.
    pub fn migrations(&self) -> &[MigrationFile] {
        &self.migrations
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

        for migration in &self.migrations {
            if applied_ids.contains(&migration.id) {
                report.skipped += 1;
                continue;
            }

            conn.apply_migration(
                migration.id,
                &migration.name,
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
