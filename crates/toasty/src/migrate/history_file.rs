use super::{err, err_ctx};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

const HISTORY_FILE_VERSION: u32 = 1;

/// Serializable record of every migration that has been generated for a
/// project.
///
/// The history file lives at `<config.path>/history.toml` and is the source of
/// truth for which migrations exist and the order they were created in. Each
/// entry is a [`HistoryFileMigration`].
///
/// The file carries a version number; loading rejects files written with an
/// incompatible version.
///
/// # Examples
///
/// ```
/// use toasty::migrate::{HistoryFile, HistoryFileMigration};
///
/// let mut history = HistoryFile::new();
/// assert_eq!(history.next_migration_number(), 0);
///
/// history.add_migration(HistoryFileMigration {
///     id: 100,
///     name: "0000_init.sql".to_string(),
///     snapshot_name: "0000_snapshot.toml".to_string(),
///     checksum: None,
/// });
/// assert_eq!(history.next_migration_number(), 1);
///
/// let serialized = history.to_string();
/// let restored: HistoryFile = serialized.parse().unwrap();
/// assert_eq!(restored.migrations()[0].id, 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFile {
    /// History file format version.
    version: u32,

    /// Generated migrations in creation order.
    migrations: Vec<HistoryFileMigration>,
}

/// A single entry in the migration history file.
///
/// Records the random ID used by the database driver to track whether the
/// migration has been applied, the SQL file name on disk, the matching
/// snapshot file name, and an optional checksum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFileMigration {
    /// Random unique identifier for this migration.
    pub id: u64,

    /// File name of the generated SQL migration, relative to
    /// [`Config::migrations_dir`](super::Config::migrations_dir).
    pub name: String,

    /// File name of the schema snapshot generated alongside this migration,
    /// relative to [`Config::snapshots_dir`](super::Config::snapshots_dir).
    pub snapshot_name: String,

    /// Optional checksum of the migration SQL file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl HistoryFile {
    /// Creates a new, empty history file.
    pub fn new() -> Self {
        Self {
            version: HISTORY_FILE_VERSION,
            migrations: Vec::new(),
        }
    }

    /// Loads a history file from `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| err_ctx(format!("reading {}", path.display()), e))?;
        contents.parse()
    }

    /// Saves the history file to `path`, overwriting any existing file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        std::fs::write(path, self.to_string())
            .map_err(|e| err_ctx(format!("writing {}", path.display()), e))?;
        Ok(())
    }

    /// Loads `path` if it exists, otherwise returns an empty history file.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        match std::fs::exists(path) {
            Ok(true) => Self::load(path),
            Ok(false) => Ok(Self::default()),
            Err(e) => Err(err_ctx(format!("checking for {}", path.display()), e)),
        }
    }

    /// Returns the recorded migrations in creation order.
    pub fn migrations(&self) -> &[HistoryFileMigration] {
        &self.migrations
    }

    /// Returns the next sequential migration number based on the last entry.
    pub fn next_migration_number(&self) -> u32 {
        self.migrations
            .last()
            .and_then(|m| m.name.split('_').next()?.parse::<u32>().ok())
            .map(|n| n + 1)
            .unwrap_or(0)
    }

    /// Appends a migration entry to the history.
    pub fn add_migration(&mut self, migration: HistoryFileMigration) {
        self.migrations.push(migration);
    }

    /// Removes the migration entry at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove_migration(&mut self, index: usize) {
        self.migrations.remove(index);
    }
}

impl Default for HistoryFile {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for HistoryFile {
    type Err = toasty_core::Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: HistoryFile =
            toml::from_str(s).map_err(|e| err_ctx("parsing migration history", e))?;

        if file.version != HISTORY_FILE_VERSION {
            return Err(err(format!(
                "unsupported history file version: {}. Expected version {}",
                file.version, HISTORY_FILE_VERSION
            )));
        }

        Ok(file)
    }
}

impl fmt::Display for HistoryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml_str = toml::to_string_pretty(self).map_err(|_| fmt::Error)?;
        f.write_str(&toml_str)
    }
}
