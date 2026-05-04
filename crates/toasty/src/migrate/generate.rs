use super::{Config, HistoryFile, HistoryFileMigration, PrefixStyle, SnapshotFile, err_ctx};
use crate::{Db, Result};
use rand::RngExt;
use std::fs;
use toasty_core::schema::db::{Migration, RenameHints, Schema, SchemaDiff};

/// Outcome of a successful [`generate`] call.
#[derive(Debug, Clone)]
pub struct Generated {
    /// File name written to [`Config::migrations_dir`](super::Config::migrations_dir).
    pub migration_name: String,

    /// File name written to [`Config::snapshots_dir`](super::Config::snapshots_dir).
    pub snapshot_name: String,

    /// History entry recorded for the new migration.
    pub entry: HistoryFileMigration,
}

/// Generates a new SQL migration from the difference between the most recent
/// snapshot and the schema currently registered on `db`.
///
/// `name` is appended after the migration prefix to produce the file name. If
/// `name` is `None`, the literal string `migration` is used.
///
/// `rename_hints` are forwarded to the schema differ. Callers that want
/// interactive rename detection (as the `toasty-cli` `generate` command
/// implements) should compute hints by repeatedly diffing
/// [`previous_schema`](super::previous_schema) against the current schema and
/// asking the user to disambiguate dropped/added pairs.
///
/// Returns `Ok(None)` if the schemas already match. Otherwise writes the
/// migration SQL, the snapshot, and the updated history file, and returns the
/// names that were written.
///
/// # Errors
///
/// Returns an error if the migration tree cannot be created or if any of the
/// file writes fail.
pub fn generate(
    db: &Db,
    config: &Config,
    name: Option<&str>,
    rename_hints: &RenameHints,
) -> Result<Option<Generated>> {
    let history_path = config.history_file_path();
    let migrations_dir = config.migrations_dir();
    let snapshots_dir = config.snapshots_dir();

    fs::create_dir_all(&migrations_dir)
        .map_err(|e| err_ctx(format!("creating {}", migrations_dir.display()), e))?;
    fs::create_dir_all(&snapshots_dir)
        .map_err(|e| err_ctx(format!("creating {}", snapshots_dir.display()), e))?;
    if let Some(parent) = history_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|e| err_ctx(format!("creating {}", parent.display()), e))?;
    }

    let mut history = HistoryFile::load_or_default(&history_path)?;

    let previous_schema = match history.migrations().last() {
        Some(latest) => SnapshotFile::load(snapshots_dir.join(&latest.snapshot_name))?.schema,
        None => Schema::default(),
    };

    let schema = Schema::clone(&db.schema().db);

    let diff = SchemaDiff::from(&previous_schema, &schema, rename_hints);
    if diff.is_empty() {
        return Ok(None);
    }

    let snapshot = SnapshotFile::new(schema.clone());
    let prefix = match config.prefix_style {
        PrefixStyle::Sequential => format!("{:04}", history.next_migration_number()),
        PrefixStyle::Timestamp => jiff::Timestamp::now().strftime("%Y%m%d_%H%M%S").to_string(),
    };
    let snapshot_name = format!("{prefix}_snapshot.toml");
    let snapshot_path = snapshots_dir.join(&snapshot_name);

    let migration_name = format!("{prefix}_{}.sql", name.unwrap_or("migration"));
    let migration_path = migrations_dir.join(&migration_name);

    let migration = db.driver().generate_migration(&diff);

    let entry = HistoryFileMigration {
        // Some databases only support signed 64-bit integers.
        id: rand::rng().random_range(0..i64::MAX) as u64,
        name: migration_name.clone(),
        snapshot_name: snapshot_name.clone(),
        checksum: None,
    };
    history.add_migration(entry.clone());

    let Migration::Sql(sql) = migration;
    fs::write(&migration_path, format!("{sql}\n"))
        .map_err(|e| err_ctx(format!("writing {}", migration_path.display()), e))?;
    snapshot.save(&snapshot_path)?;
    history.save(&history_path)?;

    Ok(Some(Generated {
        migration_name,
        snapshot_name,
        entry,
    }))
}
