use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use toasty_core::migrate::MigrationConfig;

/// Configuration for Toasty CLI operations.
///
/// Holds all settings that control how the CLI behaves. Currently this is
/// limited to [`MigrationConfig`]. A `Config` can be built programmatically
/// with the builder methods or loaded from a `Toasty.toml` file via
/// [`Config::load`].
///
/// # Examples
///
/// ```
/// use toasty_cli::{Config, MigrationConfig, MigrationPrefixStyle};
///
/// let config = Config::new()
///     .migration(
///         MigrationConfig::new()
///             .path("db")
///             .prefix_style(MigrationPrefixStyle::Timestamp),
///     );
/// assert_eq!(
///     config.migration.get_migrations_dir(),
///     std::path::PathBuf::from("db/migrations"),
/// );
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Migration-related configuration
    pub migration: MigrationConfig,
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from Toasty.toml in the project root
    pub fn load() -> Result<Self> {
        let path = Path::new("Toasty.toml");
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Set the migration configuration
    pub fn migration(mut self, migration: MigrationConfig) -> Self {
        self.migration = migration;
        self
    }
}
