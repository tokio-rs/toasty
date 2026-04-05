use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

const HISTORY_FILE_VERSION: u32 = 1;

/// A TOML-serializable record of all migrations that have been generated.
///
/// The history file lives at `<migration_path>/history.toml` and is the
/// source of truth for which migrations exist and what order they were
/// created in. Each entry is a [`HistoryFileMigration`].
///
/// The file carries a version number. [`HistoryFile::load`] and the
/// [`FromStr`] implementation reject files whose version does not match the
/// current format.
///
/// # Examples
///
/// ```
/// use toasty_core::migrate::{HistoryFile, HistoryFileMigration};
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
/// assert_eq!(history.migrations().len(), 1);
///
/// // Round-trip through TOML serialization
/// let serialized = history.to_string();
/// let restored: HistoryFile = serialized.parse().unwrap();
/// assert_eq!(restored.migrations()[0].id, 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFile {
    /// History file format version
    version: u32,

    /// Migration history
    migrations: Vec<HistoryFileMigration>,
}

/// A single entry in the migration history.
///
/// Each entry records the randomly-assigned ID used by the database driver to
/// track application status, the migration SQL file name, the companion
/// snapshot file name, and an optional checksum.
///
/// # Examples
///
/// ```
/// use toasty_core::migrate::HistoryFileMigration;
///
/// let entry = HistoryFileMigration {
///     id: 42,
///     name: "0001_create_users.sql".to_string(),
///     snapshot_name: "0001_snapshot.toml".to_string(),
///     checksum: None,
/// };
/// assert_eq!(entry.id, 42);
/// assert_eq!(entry.name, "0001_create_users.sql");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFileMigration {
    /// Random unique identifier for this migration.
    pub id: u64,

    /// Migration name/identifier.
    pub name: String,

    /// Name of the snapshot generated alongside this migration.
    pub snapshot_name: String,

    /// Optional checksum of the migration file to detect changes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl HistoryFile {
    /// Create a new empty history file
    pub fn new() -> Self {
        Self {
            version: HISTORY_FILE_VERSION,
            migrations: Vec::new(),
        }
    }

    /// Load a history file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> crate::Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            crate::Error::from_args(format_args!("migration file load failed: {e}"))
        })?;
        contents.parse()
    }

    /// Save the history file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> crate::Result<()> {
        std::fs::write(path.as_ref(), self.to_string()).map_err(|e| {
            crate::Error::from_args(format_args!("migration file save failed: {e}"))
        })?;
        Ok(())
    }

    /// Loads the history file, or returns an empty one if it does not exist
    pub fn load_or_default(path: impl AsRef<Path>) -> crate::Result<Self> {
        if path.as_ref().exists() {
            return Self::load(path);
        }
        Ok(Self::default())
    }

    /// Returns the ordered list of migrations in this history.
    ///
    /// Migrations appear in the order they were added. An empty slice means no
    /// migrations have been recorded yet.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::migrate::{HistoryFile, HistoryFileMigration};
    ///
    /// let mut history = HistoryFile::new();
    /// assert!(history.migrations().is_empty());
    ///
    /// history.add_migration(HistoryFileMigration {
    ///     id: 1,
    ///     name: "0001_init.sql".to_string(),
    ///     snapshot_name: "0001_snapshot.toml".to_string(),
    ///     checksum: None,
    /// });
    /// assert_eq!(history.migrations().len(), 1);
    /// assert_eq!(history.migrations()[0].name, "0001_init.sql");
    /// ```
    pub fn migrations(&self) -> &[HistoryFileMigration] {
        &self.migrations
    }

    /// Get the next migration number by parsing the last migration's name
    pub fn next_migration_number(&self) -> u32 {
        self.migrations
            .last()
            .and_then(|m| {
                // Extract the first 4 digits from the migration name (e.g., "0001_migration.sql" -> 1)
                m.name.split('_').next()?.parse::<u32>().ok()
            })
            .map(|n| n + 1)
            .unwrap_or(0)
    }

    /// Add a migration to the history
    pub fn add_migration(&mut self, migration: HistoryFileMigration) {
        self.migrations.push(migration);
    }

    /// Remove a migration from the history by index
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
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        let file: HistoryFile = toml::from_str(s).map_err(|e| {
            crate::Error::from_args(format_args!("migration file load failed: {e}"))
        })?;

        if file.version != HISTORY_FILE_VERSION {
            return Err(crate::Error::unsupported_migration_version(
                file.version,
                HISTORY_FILE_VERSION,
            ));
        }

        Ok(file)
    }
}

impl fmt::Display for HistoryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml_str = toml::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", toml_str)
    }
}
