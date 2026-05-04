use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use toasty::migrate::Config as MigrationConfig;

/// Configuration for Toasty CLI operations.
///
/// Holds all settings that control how the CLI behaves. Currently this is
/// limited to a [`toasty::migrate::Config`]. A `Config` can be built
/// programmatically with the builder methods or loaded from a `Toasty.toml`
/// file via [`Config::load`].
///
/// # Examples
///
/// ```
/// use toasty::migrate::{Config as MigrationConfig, PrefixStyle};
/// use toasty_cli::Config;
///
/// let config = Config::new()
///     .migration(
///         MigrationConfig::new()
///             .path("db")
///             .prefix_style(PrefixStyle::Timestamp),
///     );
/// assert_eq!(
///     config.migration.migrations_dir(),
///     std::path::PathBuf::from("db/migrations"),
/// );
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Migration-related configuration.
    pub migration: MigrationConfig,
}

impl Config {
    /// Create a new Config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from Toasty.toml in the project root
    pub fn load() -> Result<Self> {
        Self::load_from(Path::new("Toasty.toml"))
    }

    /// Load configuration from a specific path.
    pub fn load_from(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Load configuration from `<project_root>/Toasty.toml`, creating it with
    /// default contents if the file does not exist.
    pub fn load_or_default(project_root: &Path) -> Result<Self> {
        let path = project_root.join("Toasty.toml");
        if path.exists() {
            Self::load_from(&path)
        } else {
            let config = Self::default();
            let toml = toml::to_string_pretty(&config)?;
            fs::write(&path, toml)?;
            Ok(config)
        }
    }

    /// Set the migration configuration
    pub fn migration(mut self, migration: MigrationConfig) -> Self {
        self.migration = migration;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_or_default_creates_toasty_toml_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");
        assert!(!path.exists());

        Config::load_or_default(dir.path()).unwrap();

        assert!(path.exists(), "Toasty.toml should be created on first load");
        let contents = fs::read_to_string(&path).unwrap();
        let reparsed: Config = toml::from_str(&contents).unwrap();
        let default = Config::default();

        assert_eq!(reparsed, default);
    }
}
