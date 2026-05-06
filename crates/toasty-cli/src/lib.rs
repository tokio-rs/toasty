#![warn(missing_docs)]
//! Standalone CLI for Toasty.
//!
//! `toasty-cli` powers the bundled `toasty` binary. It collects the user's
//! schema by synthesizing and running an ephemeral "dumper" crate against the
//! package in the current working directory, then dispatches migration
//! subcommands:
//!
//! - `migration generate` runs offline; pick the dialect with
//!   `--flavor <sqlite|postgresql|mysql>`.
//! - `migration apply` / `reset` open a connection via `--url`.
//!
//! The crate also re-exports the underlying configuration and file types so
//! external tooling can read and manipulate migration history and snapshots
//! directly.

mod config;
mod dumper;
mod flavor;
mod migration;
mod theme;
mod utility;

pub use config::*;
pub use dumper::extract_schema;
pub use flavor::Flavor;
pub use migration::*;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use toasty::Db;

/// CLI runner for the standalone `toasty` binary.
///
/// Resolves the user's schema from the current working directory and
/// dispatches migration subcommands.
pub struct ToastyCli {
    config: Config,
    /// Project root: the cwd at construction time. Used to locate the user's
    /// package for the dumper and to write `Toasty.toml` lazily.
    project_root: PathBuf,
}

impl ToastyCli {
    /// Build a CLI runner rooted at the current working directory. Loads
    /// `Toasty.toml` if present, otherwise uses the default config.
    pub fn new() -> Result<Self> {
        let project_root = std::env::current_dir()
            .context("resolving current working directory for the toasty CLI")?;
        let config = Config::load_or_default(&project_root)
            .context("loading Toasty.toml from project root")?;
        Ok(Self {
            config,
            project_root,
        })
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Parse and execute CLI commands from `std::env::args`.
    pub async fn parse_and_run(&self) -> Result<()> {
        self.run(Cli::parse()).await
    }

    /// Parse and execute CLI commands from an iterator of arguments.
    pub async fn parse_from<I, T>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        self.run(Cli::parse_from(args)).await
    }

    async fn run(&self, cli: Cli) -> Result<()> {
        let Command::Migration(cmd) = cli.command;

        match cmd.subcommand {
            MigrationSubcommand::Generate(gen_cmd) => {
                let flavor: toasty_sql::Flavor = gen_cmd
                    .flavor()
                    .ok_or_else(|| {
                        anyhow!(
                            "--flavor <sqlite|postgresql|mysql> is required for migration generate"
                        )
                    })?
                    .into();
                let app_schema = extract_schema(&self.project_root)?;
                gen_cmd.run(app_schema, flavor, &self.config, &self.project_root)
            }
            other => {
                let url = cli.url.as_deref().ok_or_else(|| {
                    anyhow!("--url <DATABASE_URL> is required for this subcommand")
                })?;
                let app = extract_schema(&self.project_root)?;
                let db = Db::builder()
                    .app_schema(app)
                    .connect(url)
                    .await
                    .with_context(|| format!("connecting to database at {url}"))?;
                other.run_with_db(&db, &self.config).await
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Toasty CLI - Database migration and management tool")]
#[command(version)]
struct Cli {
    /// Database URL used by subcommands that connect (e.g. `migration apply`).
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
