//! The standalone `toasty` binary.
//!
//! Installed once with `cargo install toasty-cli` and run from any Cargo
//! package that uses Toasty. Unlike [`ToastyCli`](crate::ToastyCli), which a
//! project embeds in a binary of its own, this CLI links none of the user's
//! models: it reads the schema out of the artifact the project already builds.
//! See [`extract`] for how.

mod cargo;
mod extract;

use crate::{Config, migration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use toasty::migration::Flavor;

use std::path::{Path, PathBuf};

/// Runs the standalone `toasty` CLI, parsing arguments from the environment.
///
/// This is the entry point of the `toasty` binary.
///
/// # Errors
///
/// Returns an error if the command fails. Argument parsing failures exit the
/// process directly, as clap does for every CLI.
pub fn run() -> Result<()> {
    Cli::parse().run()
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Manage a Toasty project's database schema")]
// A flattened `Args` struct otherwise donates its doc comment to the command.
#[command(long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[command(flatten)]
    target: cargo::TargetFlags,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate, apply, and manage schema migrations
    #[command(subcommand)]
    Migrate(MigrateCommand),

    /// Load a cdylib so its constructors run.
    ///
    /// Internal: the CLI re-execs itself with this to dump a lib-only
    /// project's schema in a process it can afford to have exit.
    #[command(name = extract::LOAD_CDYLIB_SUBCOMMAND, hide = true)]
    LoadCdylib {
        /// The shared library to load.
        path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum MigrateCommand {
    /// Generate a migration from the changes since the last snapshot
    Generate {
        /// SQL dialect to generate for
        #[arg(long, value_name = "FLAVOR")]
        flavor: FlavorArg,

        /// Name for the migration
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Apply pending migrations to a database
    Apply {
        /// Database connection URL
        #[arg(long, value_name = "URL")]
        url: String,
    },

    /// Drop every table in a database, then re-apply the migrations
    Reset {
        /// Database connection URL
        #[arg(long, value_name = "URL")]
        url: String,

        /// Skip applying migrations after the reset
        #[arg(long)]
        skip_migrations: bool,
    },

    /// Remove a migration from the history and delete its files
    Drop(migration::DropCommand),

    /// Print the current schema as a snapshot
    Snapshot {
        /// SQL dialect to resolve the schema against
        #[arg(long, value_name = "FLAVOR")]
        flavor: FlavorArg,
    },
}

/// `--flavor`, as clap sees it.
///
/// A local enum rather than [`Flavor`] itself, which belongs to `toasty-core`
/// and so cannot implement clap's traits here. Mirroring it lets clap list the
/// choices in `--help` and reject anything else with its usual message.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum FlavorArg {
    Sqlite,
    #[value(alias = "postgres", alias = "pg")]
    Postgresql,
    #[value(alias = "mariadb")]
    Mysql,
}

impl From<FlavorArg> for Flavor {
    fn from(arg: FlavorArg) -> Self {
        match arg {
            FlavorArg::Sqlite => Self::Sqlite,
            FlavorArg::Postgresql => Self::Postgresql,
            FlavorArg::Mysql => Self::Mysql,
        }
    }
}

impl Cli {
    fn run(self) -> Result<()> {
        match self.command {
            Command::LoadCdylib { path } => extract::load_cdylib(&path),
            Command::Migrate(command) => command.run(&self.target),
        }
    }
}

impl MigrateCommand {
    fn run(self, target: &cargo::TargetFlags) -> Result<()> {
        match self {
            Self::Generate { flavor, name } => {
                let flavor = flavor.into();
                let (schema, config) = resolve_schema(target, flavor)?;
                migration::run_generate(&schema, flavor, &config, name.as_deref())
            }
            Self::Snapshot { flavor } => {
                let (schema, _) = resolve_schema(target, flavor.into())?;
                migration::run_snapshot(&schema)
            }
            Self::Drop(command) => command.run_drop(&project_config(target)?),
            // Applying and resetting only need a connection, so they skip the
            // build entirely and go straight to the URL.
            Self::Apply { url } => {
                let config = project_config(target)?;
                block_on(async {
                    let driver = toasty::db::Connect::new(&url).await?;
                    migration::run_apply(&driver, &config).await
                })
            }
            Self::Reset {
                url,
                skip_migrations,
            } => {
                let config = project_config(target)?;
                block_on(async {
                    let driver = toasty::db::Connect::new(&url).await?;
                    migration::run_reset(&driver, &config, skip_migrations).await
                })
            }
        }
    }
}

/// Extracts the project's schema and lowers it for `flavor`.
///
/// The dump is the application schema — models, fields, relations — which is
/// independent of any database. Turning it into tables and columns is where
/// the flavor comes in, because storage types and identifier limits differ.
fn resolve_schema(
    target: &cargo::TargetFlags,
    flavor: Flavor,
) -> Result<(toasty::schema::db::Schema, Config)> {
    let config = project_config(target)?;
    let app_schema = extract::schema(target)?;

    let schema = toasty_core::schema::Builder::default()
        .build(app_schema, flavor.capability())
        .context("the extracted schema is not valid for this flavor")?;

    Ok((schema.db, config))
}

/// Loads `Toasty.toml` from the target package and roots its migration paths
/// there, so the CLI reads and writes the same files from anywhere in the
/// workspace.
fn project_config(target: &cargo::TargetFlags) -> Result<Config> {
    let metadata = cargo::Metadata::load(target)?;
    let root = metadata.target_package(target)?.root_dir().to_path_buf();

    let mut config = Config::load_or_default(&root)?;
    config.migration.path = absolutize(&root, &config.migration.path);

    Ok(config)
}

fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Runs `future` on a current-thread runtime.
///
/// Only the database-facing commands are async, and they issue one statement
/// at a time; a full multi-threaded runtime would buy nothing.
fn block_on<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to start the async runtime")?
        .block_on(future)
}
