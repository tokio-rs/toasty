mod migration;

use anyhow::Result;
use clap::Parser;
use toasty::Db;

/// Toasty CLI library for building custom command-line tools
pub struct ToastyCli {
    db: Db,
}

impl ToastyCli {
    /// Create a new ToastyCli instance with the given database connection
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Parse and execute CLI commands from command-line arguments
    pub fn parse_and_run(&self) -> Result<()> {
        let cli = Cli::parse();
        self.run(cli)
    }

    /// Parse and execute CLI commands from an iterator of arguments
    pub fn parse_from<I, T>(&self, args: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let cli = Cli::parse_from(args);
        self.run(cli)
    }

    fn run(&self, cli: Cli) -> Result<()> {
        match cli.command {
            Command::Migration(cmd) => migration::run(cmd, &self.db),
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
