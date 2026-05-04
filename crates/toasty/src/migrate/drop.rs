use super::{Config, HistoryFile, err, err_ctx};
use crate::Result;
use std::fs;

/// Selects which migration entry to drop from the history.
#[derive(Debug, Clone)]
pub enum DropTarget<'a> {
    /// Drop the most recently generated migration.
    Latest,

    /// Drop the migration whose name matches `&str`.
    Name(&'a str),

    /// Drop the migration at the given index in the history file.
    Index(usize),
}

/// Outcome of a successful [`drop_migration`] call.
#[derive(Debug, Clone)]
pub struct Dropped {
    /// File name of the dropped migration SQL file.
    pub migration_name: String,

    /// File name of the snapshot associated with the dropped migration.
    pub snapshot_name: String,

    /// `true` when the migration SQL file existed on disk and was deleted.
    pub migration_file_deleted: bool,

    /// `true` when the snapshot file existed on disk and was deleted.
    pub snapshot_file_deleted: bool,
}

/// Removes a migration from the history file and deletes the associated SQL
/// and snapshot files on disk.
///
/// The database is **not** touched: any changes that have already been
/// applied will remain. This is intended for cleaning up generated files
/// before they have been applied (or after reset).
///
/// # Errors
///
/// Returns an error if the history file cannot be loaded, if the target does
/// not match an entry, or if writing the updated history file fails.
pub fn drop_migration(config: &Config, target: DropTarget<'_>) -> Result<Dropped> {
    let history_path = config.history_file_path();
    let mut history = HistoryFile::load_or_default(&history_path)?;

    if history.migrations().is_empty() {
        return Err(err("no migrations found in history"));
    }

    let index = match target {
        DropTarget::Latest => history.migrations().len() - 1,
        DropTarget::Index(index) => {
            if index >= history.migrations().len() {
                return Err(err(format!(
                    "migration index {} out of range (history has {} migrations)",
                    index,
                    history.migrations().len()
                )));
            }
            index
        }
        DropTarget::Name(name) => history
            .migrations()
            .iter()
            .position(|m| m.name == name)
            .ok_or_else(|| err(format!("migration '{name}' not found")))?,
    };

    let entry = history.migrations()[index].clone();

    let migration_path = config.migrations_dir().join(&entry.name);
    let migration_file_deleted = if migration_path.exists() {
        fs::remove_file(&migration_path)
            .map_err(|e| err_ctx(format!("deleting {}", migration_path.display()), e))?;
        true
    } else {
        false
    };

    let snapshot_path = config.snapshots_dir().join(&entry.snapshot_name);
    let snapshot_file_deleted = if snapshot_path.exists() {
        fs::remove_file(&snapshot_path)
            .map_err(|e| err_ctx(format!("deleting {}", snapshot_path.display()), e))?;
        true
    } else {
        false
    };

    history.remove_migration(index);
    history.save(&history_path)?;

    Ok(Dropped {
        migration_name: entry.name,
        snapshot_name: entry.snapshot_name,
        migration_file_deleted,
        snapshot_file_deleted,
    })
}
