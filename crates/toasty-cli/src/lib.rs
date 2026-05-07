#![warn(missing_docs)]
//! Standalone CLI for Toasty. Powers the bundled `toasty` binary: extracts
//! the user's schema via the dumper crate and dispatches migration
//! subcommands. `generate` runs offline (`--flavor`); `apply`/`reset` connect
//! via `--url`.

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

        let app_schema = extract_schema(&self.project_root)?;

        // Built lazily so `generate` doesn't require `--url`.
        let connect = || async {
            let url = cli
                .url
                .as_deref()
                .ok_or_else(|| anyhow!("--url <DATABASE_URL> is required for this subcommand"))?;
            Db::builder()
                .app_schema(app_schema.clone())
                .connect(url)
                .await
                .with_context(|| format!("connecting to database at {url}"))
        };

        match cmd.subcommand {
            MigrationSubcommand::Generate(c) => c.run(app_schema, &self.config, &self.project_root),
            MigrationSubcommand::Apply(c) => c.run(&connect().await?, &self.config).await,
            MigrationSubcommand::Snapshot(c) => c.run(&connect().await?, &self.config),
            MigrationSubcommand::Drop(c) => c.run(&connect().await?, &self.config),
            MigrationSubcommand::Reset(c) => c.run(&connect().await?, &self.config).await,
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
