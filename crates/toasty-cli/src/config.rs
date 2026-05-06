use crate::migration::MigrationConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
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
        Self::load_from(Path::new("Toasty.toml"))
    }

    /// Load configuration from a specific path.
    pub fn load_from(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Load configuration from `<project_root>/Toasty.toml`, falling back to
    /// the default config (without touching disk) if the file is missing.
    /// The file is written lazily by [`Config::save_if_missing`] when the
    /// first migration is generated, so just running the CLI doesn't drop
    /// a Toasty.toml into the user's project.
    pub fn load_or_default(project_root: &Path) -> Result<Self> {
        let path = project_root.join("Toasty.toml");
        if path.exists() {
            Self::load_from(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Write `<project_root>/Toasty.toml` with this config's contents, but
    /// only if the file does not already exist.
    pub fn save_if_missing(&self, project_root: &Path) -> Result<()> {
        let path = project_root.join("Toasty.toml");
        if path.exists() {
            return Ok(());
        }
        let toml = toml::to_string_pretty(self)?;
        fs::write(&path, toml)?;
        Ok(())
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
    fn load_or_default_does_not_create_toasty_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");
        assert!(!path.exists());

        let config = Config::load_or_default(dir.path()).unwrap();

        assert!(!path.exists(), "Toasty.toml must not be created on load");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn save_if_missing_writes_when_absent_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");

        Config::default().save_if_missing(dir.path()).unwrap();
        assert!(path.exists());

        // Modify the file; a subsequent save must not overwrite it.
        fs::write(&path, "# user edits\n").unwrap();
        Config::default().save_if_missing(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# user edits\n");
    }
}
