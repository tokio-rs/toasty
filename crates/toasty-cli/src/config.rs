use std::path::PathBuf;

/// Configuration for Toasty CLI operations
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to the migrations folder
    pub migrations_path: PathBuf,

    /// Style of migration file prefixes
    pub migration_prefix_style: MigrationPrefixStyle,
}

/// Style for migration file name prefixes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationPrefixStyle {
    /// Sequential numbering (e.g., 0001_, 0002_, 0003_)
    Sequential,

    /// Timestamp-based (e.g., 20240112_153045_)
    Timestamp,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            migrations_path: PathBuf::from("migrations"),
            migration_prefix_style: MigrationPrefixStyle::Sequential,
        }
    }
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the migrations path
    pub fn migrations_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.migrations_path = path.into();
        self
    }

    /// Set the migration prefix style
    pub fn migration_prefix_style(mut self, style: MigrationPrefixStyle) -> Self {
        self.migration_prefix_style = style;
        self
    }
}
