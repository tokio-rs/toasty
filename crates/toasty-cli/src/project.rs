use crate::cargo::{BuildTarget, Metadata};
use crate::config::Config;

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// The Cargo package the CLI operates on, plus its Toasty configuration.
///
/// All migration files live under the package's directory (`Toasty.toml`,
/// `toasty/`), so workspace members keep independent migration histories.
pub struct Project {
    /// Name of the selected package.
    pub package_name: String,

    /// Directory containing the package's `Cargo.toml`.
    pub package_root: PathBuf,

    /// The workspace root directory.
    pub workspace_root: PathBuf,

    /// Whether the package declares a direct dependency on `toasty`.
    pub depends_on_toasty: bool,

    /// Bin target names declared by the package.
    pub bin_names: Vec<String>,

    /// Whether the package has a lib target.
    pub has_lib: bool,

    /// Configuration loaded from the package's `Toasty.toml` (or defaults).
    pub config: Config,
}

impl Project {
    /// Locates the target package via `cargo metadata` and loads its
    /// configuration.
    pub fn locate(package: Option<&str>) -> Result<Self> {
        let metadata = Metadata::load()?;
        let pkg = metadata.select_package(package)?;

        let package_root = pkg.root();
        let config = Config::load_or_default(&package_root)?;

        Ok(Project {
            package_name: pkg.name().to_string(),
            package_root,
            workspace_root: PathBuf::from(metadata.workspace_root()),
            depends_on_toasty: pkg.depends_on_toasty(),
            bin_names: pkg.bin_names().iter().map(|s| s.to_string()).collect(),
            has_lib: pkg.has_lib(),
            config,
        })
    }

    /// Picks the artifact to build for schema extraction.
    ///
    /// A bin target is preferred; lib-only packages fall back to building the
    /// lib as a `cdylib`. `bin` selects among multiple bin targets.
    pub fn build_target(&self, bin: Option<&str>) -> Result<BuildTarget> {
        if let Some(name) = bin {
            if !self.bin_names.iter().any(|b| b == name) {
                bail!(
                    "package `{}` has no bin target named `{name}`; available bins: {}",
                    self.package_name,
                    self.bin_names.join(", ")
                );
            }
            return Ok(BuildTarget::Bin(name.to_string()));
        }

        match self.bin_names.as_slice() {
            [] if self.has_lib => Ok(BuildTarget::Cdylib),
            [] => bail!(
                "package `{}` has no bin or lib target to extract a schema from",
                self.package_name
            ),
            [only] => Ok(BuildTarget::Bin(only.clone())),
            many => bail!(
                "package `{}` has multiple bin targets; select one with `--bin <name>`: {}",
                self.package_name,
                many.join(", ")
            ),
        }
    }

    /// Directory holding generated migration SQL files.
    pub fn migrations_dir(&self) -> PathBuf {
        self.package_root
            .join(self.config.migration.get_migrations_dir())
    }

    /// Directory holding schema snapshot files.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.package_root
            .join(self.config.migration.get_snapshots_dir())
    }

    /// Path of the migration history file.
    pub fn history_file_path(&self) -> PathBuf {
        self.package_root
            .join(self.config.migration.get_history_file_path())
    }

    /// The flavor to use: the command-line flag when given, otherwise the
    /// `migration.flavor` entry in `Toasty.toml`.
    pub fn flavor(&self, flag: Option<crate::Flavor>) -> Result<crate::Flavor> {
        flag.or(self.config.migration.flavor).context(
            "no flavor selected; pass `--flavor <sqlite|postgresql|mysql|turso>` or set \
             `migration.flavor` in Toasty.toml",
        )
    }
}
