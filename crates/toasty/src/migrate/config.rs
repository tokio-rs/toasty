use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the on-disk layout and naming of migrations.
///
/// `Config` controls where migration SQL files, schema snapshots, and the
/// history file are written, and how new migration file names are prefixed.
/// The default uses a `toasty/` base directory with sequential numeric
/// prefixes.
///
/// # Examples
///
/// ```
/// use toasty::migrate::{Config, PrefixStyle};
///
/// let config = Config::new()
///     .path("db")
///     .prefix_style(PrefixStyle::Timestamp);
///
/// assert_eq!(config.migrations_dir(), std::path::Path::new("db/migrations"));
/// assert_eq!(config.snapshots_dir(), std::path::Path::new("db/snapshots"));
/// assert_eq!(config.history_file_path(), std::path::Path::new("db/history.toml"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Base directory under which migrations, snapshots, and history live.
    pub path: PathBuf,

    /// Format used to prefix newly generated migration file names.
    pub prefix_style: PrefixStyle,

    /// Whether the history file should record and verify a checksum of each
    /// migration file. Reserved for future use; currently unused.
    pub checksums: bool,

    /// Whether to insert statement breakpoint comments into generated SQL
    /// migrations. Reserved for future use; currently unused.
    pub statement_breakpoints: bool,
}

/// Format of the prefix prepended to generated migration file names.
///
/// The prefix determines the on-disk ordering of migration files and the
/// numbering used when displaying migrations to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrefixStyle {
    /// Sequential numbering starting at `0000_` and incrementing for each
    /// generated migration.
    Sequential,

    /// Timestamp-based prefix in `YYYYMMDD_HHMMSS_` form, derived from the
    /// system clock at generation time.
    Timestamp,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: PathBuf::from("toasty"),
            prefix_style: PrefixStyle::Sequential,
            checksums: false,
            statement_breakpoints: true,
        }
    }
}

impl Config {
    /// Returns a `Config` populated with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the base directory.
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    /// Sets the prefix style used when generating migration file names.
    pub fn prefix_style(mut self, style: PrefixStyle) -> Self {
        self.prefix_style = style;
        self
    }

    /// Returns the directory where migration SQL files are stored.
    pub fn migrations_dir(&self) -> PathBuf {
        self.path.join("migrations")
    }

    /// Returns the directory where schema snapshot files are stored.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.path.join("snapshots")
    }

    /// Returns the path to the migration history file.
    pub fn history_file_path(&self) -> PathBuf {
        self.path.join("history.toml")
    }
}
