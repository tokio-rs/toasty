use crate::migrate::MigrationConfig;
use anyhow::{Context, Result};
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
        let contents = fs::read_to_string(path).with_context(|| {
            format!(
                "failed to read Toasty config file at `{}` — check that the file exists \
                 at this path relative to the working directory (a common cause in Docker \
                 multi-stage builds is forgetting to copy it into the final image)",
                path.display()
            )
        })?;
        let config: Config = toml::from_str(&contents).with_context(|| {
            format!("failed to parse Toasty config file at `{}`", path.display())
        })?;
        Ok(config)
    }

    /// Load configuration from `<project_root>/Toasty.toml`, falling back to
    /// the defaults when the file does not exist.
    ///
    /// This does not create the file. Every subcommand loads the config, but
    /// only `migrate generate` writes to the source tree — so `migrate apply`
    /// and `migrate drop` keep working against a read-only checkout.
    pub fn load_or_default(project_root: &Path) -> Result<Self> {
        let path = project_root.join("Toasty.toml");
        if path.exists() {
            Self::load_from(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Writes a default `Toasty.toml` into `project_root` if none is there.
    ///
    /// Called by the subcommands that already write to the source tree, so
    /// that a project picks up a config file on its first `migrate generate`.
    pub fn create_if_missing(project_root: &Path) -> Result<()> {
        let path = project_root.join("Toasty.toml");

        if path.exists() {
            return Ok(());
        }

        let toml = toml::to_string_pretty(&Self::default())?;
        fs::write(&path, toml).with_context(|| format!("failed to write `{}`", path.display()))?;

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
    fn load_or_default_does_not_write_toasty_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");

        let config = Config::load_or_default(dir.path()).unwrap();

        assert_eq!(config, Config::default());
        assert!(
            !path.exists(),
            "loading the config must not write to the source tree"
        );
    }

    #[test]
    fn create_if_missing_writes_defaults_then_leaves_them_alone() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");

        Config::create_if_missing(dir.path()).unwrap();

        assert!(path.exists(), "Toasty.toml should be created when absent");
        let reparsed: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(reparsed, Config::default());

        // An existing file is left untouched, including settings that differ
        // from the defaults.
        fs::write(
            &path,
            "[migration]\npath = \"db\"\nprefix_style = \"Timestamp\"\n",
        )
        .unwrap();
        Config::create_if_missing(dir.path()).unwrap();

        let config = Config::load_or_default(dir.path()).unwrap();
        assert_eq!(config.migration.path, std::path::PathBuf::from("db"));
    }

    #[test]
    fn load_ignores_removed_config_keys() {
        // Toasty.toml files written by earlier versions contain keys that no
        // longer exist; they must still parse.
        let config: Config = toml::from_str(
            "[migration]\npath = \"toasty\"\nprefix_style = \"Sequential\"\n\
             checksums = false\nstatement_breakpoints = true\n",
        )
        .unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn load_from_missing_file_reports_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Toasty.toml");

        let err = Config::load_from(&path).unwrap_err();

        let message = format!("{err:#}");
        assert!(
            message.contains(&path.display().to_string()),
            "error message should mention the missing path: {message}"
        );
    }
}
