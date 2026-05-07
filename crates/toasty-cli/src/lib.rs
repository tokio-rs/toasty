#![warn(missing_docs)]
//! Standalone CLI for Toasty. Powers the bundled `toasty` binary and
//! dispatches migration subcommands. Only `generate` invokes the dumper to
//! extract the user's `app::Schema`; `apply`/`reset` open the database via
//! `--url` (no schema needed — they run raw SQL); `snapshot`/`drop` are
//! purely file ops.

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

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use toasty::db::Connect;

/// CLI runner rooted at a project directory.
pub struct ToastyCli {
    config: Config,
    /// Project root (cwd at construction).
    project_root: PathBuf,
}

impl ToastyCli {
    /// Build a runner from the current working directory. Loads `Toasty.toml`
    /// if present, otherwise uses defaults.
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

    /// Get the loaded configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Parse `std::env::args` and run.
    pub async fn parse_and_run(&self) -> Result<()> {
        self.run(Cli::parse()).await
    }

    async fn run(&self, cli: Cli) -> Result<()> {
        let Command::Migration(cmd) = cli.command;

        // Only `generate` needs the user's models. Everything else operates on
        // saved snapshot/history files or raw SQL, so no dumper invocation.
        match cmd.subcommand {
            MigrationSubcommand::Generate(c) => {
                let app_schema = extract_schema(&self.project_root)?;
                c.run(app_schema, &self.config, &self.project_root)
            }
            MigrationSubcommand::Apply(c) => c.run(&self.config).await,
            MigrationSubcommand::Drop(c) => c.run(&self.config),
            MigrationSubcommand::Reset(c) => c.run(&self.config).await,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Toasty CLI - Database migration and management tool")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Database migration commands
    Migration(migration::MigrationCommand),
}

/// Shared `--url` argument for subcommands that open a database connection.
#[derive(Parser, Debug)]
pub(crate) struct ConnectArgs {
    /// Database URL (e.g. `sqlite://...`, `postgres://...`).
    #[arg(long)]
    url: String,
}

impl ConnectArgs {
    /// Open a raw [`Connect`] driver. No schema is needed — the migration
    /// subcommands that connect (apply/reset) talk to the driver directly via
    /// `Driver::connect` and never read `Db::schema`.
    pub(crate) async fn driver(&self) -> Result<Connect> {
        Connect::new(&self.url)
            .await
            .with_context(|| format!("connecting to database at {}", self.url))
    }
}
