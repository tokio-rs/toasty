use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for migration operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfig {
    /// Path to the migrations folder
    pub path: PathBuf,

    /// Style of migration file prefixes
    pub prefix_style: MigrationPrefixStyle,

    /// Whether the history file should store and verify checksums of the migration files so that
    /// they may not be changed.
    pub checksums: bool,

    /// Whether to add statement breakpoint comments to generated SQL migration files.
    /// These comments mark boundaries where SQL statements should be split for execution.
    /// This is needed because different databases have different batching capabilities:
    /// some (like PostgreSQL) can execute multiple statements in one batch, while others
    /// require each statement to be executed separately.
    pub statement_breakpoints: bool,
}

/// Style for migration file name prefixes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationPrefixStyle {
    /// Sequential numbering (e.g., 0001_, 0002_, 0003_)
    Sequential,

    /// Timestamp-based (e.g., 20240112_153045_)
    Timestamp,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("toasty"),
            prefix_style: MigrationPrefixStyle::Sequential,
            checksums: false,
            statement_breakpoints: true,
        }
    }
}

impl MigrationConfig {
    /// Create a new MigrationConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the migrations path
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the migration prefix style
    pub fn prefix_style(mut self, style: MigrationPrefixStyle) -> Self {
        self.prefix_style = style;
        self
    }

    /// Returns the directory of the migration files derived from `path`.
    pub fn get_migrations_dir(&self) -> PathBuf {
        self.path.join("migrations")
    }

    /// Returns the directory of the snapshot files derived from `path`.
    pub fn get_snapshots_dir(&self) -> PathBuf {
        self.path.join("snapshots")
    }

    /// Get the path to the history file
    pub fn get_history_file_path(&self) -> PathBuf {
        self.path.join("history.toml")
    }
}
