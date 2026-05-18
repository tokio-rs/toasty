use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

const HISTORY_VERSION: u32 = 1;

/// A TOML-serializable record of all migrations that have been generated.
///
/// The history file lives at `<migration_path>/history.toml` and is the
/// source of truth for which migrations exist and what order they were
/// created in. Each entry is a [`HistoryEntry`].
///
/// The file carries a version number. [`History::load`] and the [`FromStr`]
/// implementation reject files whose version does not match the current
/// format.
///
/// # Examples
///
/// ```
/// use toasty::migration::{History, HistoryEntry};
///
/// let mut history = History::new();
/// assert_eq!(history.next_migration_number(), 0);
///
/// history.add_entry(HistoryEntry {
///     id: 100,
///     name: "0000_init.sql".to_string(),
///     snapshot_name: "0000_snapshot.toml".to_string(),
///     checksum: None,
/// });
/// assert_eq!(history.next_migration_number(), 1);
/// assert_eq!(history.entries().len(), 1);
///
/// // Round-trip through TOML serialization
/// let serialized = history.to_string();
/// let restored: History = serialized.parse().unwrap();
/// assert_eq!(restored.entries()[0].id, 100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct History {
    /// History file format version
    version: u32,

    /// Migration history
    #[serde(rename = "migrations")]
    entries: Vec<HistoryEntry>,
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
/// use toasty::migration::HistoryEntry;
///
/// let entry = HistoryEntry {
///     id: 42,
///     name: "0001_create_users.sql".to_string(),
///     snapshot_name: "0001_snapshot.toml".to_string(),
///     checksum: None,
/// };
/// assert_eq!(entry.id, 42);
/// assert_eq!(entry.name, "0001_create_users.sql");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Random unique identifier for this migration.
    pub id: u64,

    /// Migration name/identifier.
    pub name: String,

    /// Name of the snapshot generated alongside this migration.
    pub snapshot_name: String,

    /// Optional checksum of the migration file to detect changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl History {
    /// Create a new empty history.
    pub fn new() -> Self {
        Self {
            version: HISTORY_VERSION,
            entries: Vec::new(),
        }
    }

    /// Load history from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Save the history to a TOML file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())?;
        Ok(())
    }

    /// Loads the history file, or returns an empty one if it does not exist.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if std::fs::exists(path)? {
            return Self::load(path);
        }
        Ok(Self::default())
    }

    /// Returns the ordered list of entries in this history.
    ///
    /// Entries appear in the order they were added. An empty slice means no
    /// migrations have been recorded yet.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty::migration::{History, HistoryEntry};
    ///
    /// let mut history = History::new();
    /// assert!(history.entries().is_empty());
    ///
    /// history.add_entry(HistoryEntry {
    ///     id: 1,
    ///     name: "0001_init.sql".to_string(),
    ///     snapshot_name: "0001_snapshot.toml".to_string(),
    ///     checksum: None,
    /// });
    /// assert_eq!(history.entries().len(), 1);
    /// assert_eq!(history.entries()[0].name, "0001_init.sql");
    /// ```
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Get the next migration number by parsing the last entry's name.
    pub fn next_migration_number(&self) -> u32 {
        self.entries
            .last()
            .and_then(|m| m.name.split('_').next()?.parse::<u32>().ok())
            .map(|n| n + 1)
            .unwrap_or(0)
    }

    /// Add an entry to the history.
    pub fn add_entry(&mut self, entry: HistoryEntry) {
        self.entries.push(entry);
    }

    /// Remove an entry from the history by index.
    pub fn remove_entry(&mut self, index: usize) {
        self.entries.remove(index);
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for History {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let history: History =
            toml::from_str(s).map_err(|err| Error::from_args(format_args!("{err}")))?;

        if history.version != HISTORY_VERSION {
            return Err(Error::from_args(format_args!(
                "unsupported history file version: {}. Expected version {}",
                history.version, HISTORY_VERSION
            )));
        }

        Ok(history)
    }
}

impl fmt::Display for History {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let toml_str = toml::to_string_pretty(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", toml_str)
    }
}
