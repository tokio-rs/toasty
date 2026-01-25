mod config;
mod migration;
mod theme;

pub use config::*;
pub use migration::*;

use anyhow::Result;
use clap::Parser;
use toasty::Db;

/// Toasty CLI library for building custom command-line tools
pub struct ToastyCli {
    db: Db,
    config: Config,
}

impl ToastyCli {
    /// Create a new ToastyCli instance with the given database connection
    pub fn new(db: Db) -> Self {
        Self {
            db,
            config: Config::default(),
        }
    }

    /// Create a new ToastyCli instance with a custom configuration
    pub fn with_config(db: Db, config: Config) -> Self {
        Self { db, config }
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
        match cli.command {
            Command::Migration(cmd) => cmd.run(&self.db, &self.config).await,
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
