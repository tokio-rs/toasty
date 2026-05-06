#![warn(missing_docs)]
//! A library for building Toasty command-line tools.
//!
//! `toasty-cli` provides [`ToastyCli`], a ready-made CLI runner that exposes
//! migration subcommands (generate, apply, drop, reset, snapshot). It uses
//! [clap] for argument parsing and [dialoguer] for interactive prompts.
//!
//! Two modes are supported:
//!
//! - **Library mode** ([`ToastyCli::new`]): the caller already has a
//!   [`toasty::Db`] handle (with models registered) and wants the standard
//!   migration subcommands wired up to it.
//! - **Standalone mode** ([`ToastyCli::standalone`]): used by the bundled
//!   `toasty` binary. The schema is collected by synthesizing and running an
//!   ephemeral "dumper" crate against the user's project. The CLI then takes
//!   a `--url` flag to construct the [`Db`] (which supplies the SQL flavor
//!   for migration generation and the live connection for migration apply).
//!
//! The crate also exposes the underlying configuration and file types so that
//! custom tooling can read and manipulate migration history and snapshots
//! directly.

mod config;
mod dumper;
mod migration;
mod theme;
mod utility;

pub use config::*;
pub use dumper::extract_schema;
pub use migration::*;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use toasty::Db;

/// A CLI runner that dispatches migration subcommands.
///
/// In library mode it wraps a caller-provided [`Db`]. In standalone mode it
/// resolves the schema from the user's project (via the `dumper` module) and
/// builds a [`Db`] from the `--url` CLI flag.
pub struct ToastyCli {
    db: Option<Db>,
    config: Config,
    /// Project root used to resolve the user's package in standalone mode.
    /// Set to the current working directory by [`ToastyCli::standalone`].
    project_root: Option<PathBuf>,
}

impl ToastyCli {
    /// Create a new ToastyCli instance with the given database connection.
    ///
    /// Use this when your application has already constructed a [`Db`] via
    /// `Db::builder().models(...)`. The `--url` CLI flag is rejected in this
    /// mode (the caller's `Db` already supplies the connection).
    pub fn new(db: Db) -> Self {
        Self {
            db: Some(db),
            config: Config::default(),
            project_root: None,
        }
    }

    /// Create a new ToastyCli instance for standalone use.
    ///
    /// Used by the bundled `toasty` binary. The schema is collected by
    /// synthesizing and running a "dumper" crate against the package rooted
    /// at the current working directory; the [`Db`] is constructed from the
    /// `--url` CLI flag using the dumped schema.
    pub fn standalone() -> Result<Self> {
        let project_root = std::env::current_dir()
            .context("resolving current working directory for standalone CLI")?;
        let config = Config::load_or_default(&project_root)
            .context("loading Toasty.toml from project root")?;
        Ok(Self {
            db: None,
            config,
            project_root: Some(project_root),
        })
    }

    /// Create a new ToastyCli instance with a custom configuration
    pub fn with_config(db: Db, config: Config) -> Self {
        Self {
            db: Some(db),
            config,
            project_root: None,
        }
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Parse and execute CLI commands from command-line arguments
    pub async fn parse_and_run(&self) -> Result<()> {
        let cli = Cli::parse();
        self.run(cli).await
    }

    /// Parse and execute CLI commands from an iterator of arguments
    pub async fn parse_from<I, T>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let cli = Cli::parse_from(args);
        self.run(cli).await
    }

    async fn run(&self, cli: Cli) -> Result<()> {
        let db_owned;
        let db = match &self.db {
            Some(db) => {
                if cli.url.is_some() {
                    return Err(anyhow!(
                        "--url is only valid in standalone mode; the calling library already provided a Db"
                    ));
                }
                db
            }
            None => {
                let url = cli.url.as_deref().ok_or_else(|| {
                    anyhow!("--url <DATABASE_URL> is required in standalone mode")
                })?;
                let project_root = self
                    .project_root
                    .as_deref()
                    .ok_or_else(|| anyhow!("standalone mode requires a project root"))?;

                let app = extract_schema(project_root)?;
                db_owned = Db::builder()
                    .app_schema(app)
                    .connect(url)
                    .await
                    .with_context(|| format!("connecting to database at {url}"))?;
                &db_owned
            }
        };

        match cli.command {
            Command::Migration(cmd) => {
                cmd.run(db, &self.config, self.project_root.as_deref())
                    .await
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Toasty CLI - Database migration and management tool")]
#[command(version)]
struct Cli {
    /// Database URL (standalone mode only — selects the SQL flavor and the
    /// connection used by `migration apply`).
    #[arg(long, global = true)]
    url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Database migration commands
    Migration(migration::MigrationCommand),
}
