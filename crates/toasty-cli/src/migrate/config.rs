use crate::Flavor;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for migration operations.
///
/// Controls where migration files, snapshot files, and the history file are
/// stored, how migration file names are prefixed (sequential numbers or
/// timestamps), and which database flavor migrations target by default.
///
/// All paths are relative to the target package's directory.
///
/// # Examples
///
/// ```
/// use toasty_cli::{MigrationConfig, MigrationPrefixStyle};
///
/// let config = MigrationConfig::new()
///     .path("my_app/db")
///     .prefix_style(MigrationPrefixStyle::Timestamp);
///
/// assert_eq!(
///     config.get_migrations_dir(),
///     std::path::PathBuf::from("my_app/db/migrations"),
/// );
/// assert_eq!(
///     config.get_snapshots_dir(),
///     std::path::PathBuf::from("my_app/db/snapshots"),
/// );
/// assert_eq!(
///     config.get_history_file_path(),
///     std::path::PathBuf::from("my_app/db/history.toml"),
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationConfig {
    /// Path to the migrations folder
    pub path: PathBuf,

    /// Style of migration file prefixes
    pub prefix_style: MigrationPrefixStyle,

    /// Database flavor migrations target when `--flavor` is not passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<Flavor>,
}

/// Controls the prefix format used when naming generated migration files.
///
/// The prefix appears at the start of the migration file name and determines
/// the ordering of migration files on disk.
///
/// # Examples
///
/// ```
/// use toasty_cli::MigrationPrefixStyle;
///
/// // Default is sequential
/// let style = MigrationPrefixStyle::Sequential;
/// assert_eq!(style, MigrationPrefixStyle::Sequential);
/// ```
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
            flavor: None,
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

    /// Set the default database flavor
    pub fn flavor(mut self, flavor: Flavor) -> Self {
        self.flavor = Some(flavor);
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
