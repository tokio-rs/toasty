#![warn(missing_docs)]
//! A library for building Toasty command-line tools.
//!
//! `toasty-cli` provides [`ToastyCli`], a ready-made CLI runner that wraps a
//! [`toasty::Db`] handle and exposes database migration subcommands (generate,
//! apply, drop, reset, snapshot). It uses [clap] for argument parsing and
//! [dialoguer] for interactive prompts.
//!
//! The library bits — types like
//! [`HistoryFile`](toasty::migrate::HistoryFile),
//! [`SnapshotFile`](toasty::migrate::SnapshotFile), and the
//! [`apply`](toasty::migrate::apply) / [`generate`](toasty::migrate::generate)
//! functions — live in the [`toasty::migrate`] module. This crate is the
//! interactive front-end on top of them.
//!
//! # Main types
//!
//! - [`ToastyCli`] — parses CLI arguments and dispatches to the appropriate
//!   migration subcommand.
//! - [`Config`] — configures the CLI. Wraps a
//!   [`toasty::migrate::Config`]. Loaded from a `Toasty.toml` file or built
//!   programmatically.
//!
//! # Examples
//!
//! ```ignore
//! use toasty_cli::ToastyCli;
//!
//! let db = toasty::Db::builder("sqlite::memory:").build().await?;
//! let cli = ToastyCli::new(db);
//! cli.parse_and_run().await?;
//! ```

mod config;
mod migration;
mod theme;
mod utility;

pub use config::*;
pub use migration::*;

use anyhow::Result;
use clap::Parser;
use toasty::Db;

/// A CLI runner that dispatches migration subcommands against a [`Db`].
///
/// `ToastyCli` holds a database connection and a [`Config`]. Call
/// [`parse_and_run`](Self::parse_and_run) to parse `std::env::args` and
/// execute the matching subcommand, or [`parse_from`](Self::parse_from) to
/// parse from an arbitrary iterator (useful for testing).
///
/// # Examples
///
/// ```ignore
/// use toasty::migrate::{Config as MigrationConfig, PrefixStyle};
/// use toasty_cli::{Config, ToastyCli};
///
/// let config = Config::new()
///     .migration(MigrationConfig::new().path("db"));
/// let db = toasty::Db::builder("sqlite::memory:").build().await?;
/// let cli = ToastyCli::with_config(db, config);
/// cli.parse_from(["toasty", "migration", "apply"]).await?;
/// ```
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
