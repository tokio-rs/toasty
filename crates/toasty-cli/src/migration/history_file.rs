use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

const HISTORY_FILE_VERSION: u32 = 1;

/// History file containing the record of all applied migrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFile {
    /// History file format version
    version: u32,

    /// Migration history
    migrations: Vec<HistoryFileMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryFileMigration {
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

impl HistoryFile {
    /// Create a new empty history file
    pub fn new() -> Self {
        Self {
            version: HISTORY_FILE_VERSION,
            migrations: Vec::new(),
        }
    }

    /// Load a history file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Save the history file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())?;
        Ok(())
    }

    /// Loads the history file, or returns an empty one if it does not exist
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self> {
        if std::fs::exists(&path)? {
            return Self::load(path);
        }
        Ok(Self::default())
    }

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
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let file: HistoryFile = toml::from_str(s)?;

        // Validate version
        if file.version != HISTORY_FILE_VERSION {
            bail!(
                "Unsupported history file version: {}. Expected version {}",
                file.version,
                HISTORY_FILE_VERSION
            );
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
