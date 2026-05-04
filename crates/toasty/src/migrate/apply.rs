use super::{Config, HistoryFile, HistoryFileMigration, err_ctx};
use crate::{Db, Result};
use hashbrown::HashSet;
use std::fs;
use toasty_core::schema::db::Migration;

/// Returns the migrations recorded in the history file that have not yet been
/// applied to the database, in declaration order.
///
/// An empty result means the database is up to date with the history file.
/// This function opens (and immediately releases) a single driver connection
/// to query the applied-migration ledger.
pub async fn pending(db: &Db, config: &Config) -> Result<Vec<HistoryFileMigration>> {
    let history = HistoryFile::load_or_default(config.history_file_path())?;
    let mut conn = db.driver().connect().await?;
    let applied = conn.applied_migrations().await?;
    let applied: HashSet<u64> = applied.iter().map(|m| m.id()).collect();

    Ok(history
        .migrations()
        .iter()
        .filter(|m| !applied.contains(&m.id))
        .cloned()
        .collect())
}

/// Applies every pending migration to the database, in order.
///
/// All migrations execute on a single driver connection. The returned vector
/// lists every migration that was applied; an empty vector means the database
/// was already up to date.
///
/// # Errors
///
/// Returns an error if reading the history file fails, if a migration SQL
/// file cannot be read, or if the driver fails to apply a migration.
pub async fn apply(db: &Db, config: &Config) -> Result<Vec<HistoryFileMigration>> {
    let history = HistoryFile::load_or_default(config.history_file_path())?;

    if history.migrations().is_empty() {
        return Ok(Vec::new());
    }

    let mut conn = db.driver().connect().await?;
    let already_applied = conn.applied_migrations().await?;
    let already_applied: HashSet<u64> = already_applied.iter().map(|m| m.id()).collect();

    let pending: Vec<HistoryFileMigration> = history
        .migrations()
        .iter()
        .filter(|m| !already_applied.contains(&m.id))
        .cloned()
        .collect();

    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let migrations_dir = config.migrations_dir();
    for entry in &pending {
        let path = migrations_dir.join(&entry.name);
        let sql = fs::read_to_string(&path)
            .map_err(|e| err_ctx(format!("reading {}", path.display()), e))?;
        let migration = Migration::new_sql(sql);
        conn.apply_migration(entry.id, &entry.name, &migration)
            .await?;
    }

    Ok(pending)
}
