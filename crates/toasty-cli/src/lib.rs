//! The standalone `toasty` command-line tool.
//!
//! Installed with `cargo install toasty-cli` and run from any Cargo package
//! that uses Toasty. Schema-bound commands build the package's bin (or its
//! lib as a `cdylib`) and run it with `TOASTY_DUMP_SCHEMA` set, which makes a
//! constructor inside `toasty` dump the schema and exit. See
//! `docs/dev/design/standalone-cli.md`.
//!
//! The library exists for the binary and its tests; its API is not stable.

mod cargo;
mod config;
mod extract;
mod flavor;
mod load_cdylib;
mod migrate;
mod project;
mod theme;
mod utility;

pub use cargo::Features;
pub use config::Config;
pub use flavor::Flavor;
pub use migrate::{MigrationConfig, MigrationPrefixStyle};
pub use project::Project;

use anyhow::{Context, Result};
use clap::Parser;

/// Parses `std::env::args` and runs the matching subcommand.
pub fn run() -> Result<()> {
    run_parsed(Cli::parse())
}

/// Parses from an explicit argument list and runs the matching subcommand.
/// Useful for testing.
pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    run_parsed(Cli::parse_from(args))
}

fn run_parsed(cli: Cli) -> Result<()> {
    match cli.command {
        // Runs before the async runtime exists: the subcommand mutates its
        // environment, which is only sound while single-threaded.
        Command::LoadCdylib(cmd) => cmd.run(),
        Command::Migrate(cmd) => {
            let project = Project::locate(cli.package.as_deref(), cli.features)?;
            runtime()?.block_on(cmd.run(&project))
        }
    }
}

/// Built here rather than with `#[tokio::main]` so `__load-cdylib` runs
/// before any worker thread exists.
fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start the async runtime")
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Toasty schema management and migrations")]
#[command(version)]
struct Cli {
    /// Package to operate on, when the workspace has more than one
    #[arg(short, long, global = true, value_name = "PKG")]
    package: Option<String>,

    #[command(flatten)]
    features: Features,

    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Database migration commands
    Migrate(migrate::MigrateCommand),

    /// Internal: load a cdylib so its schema-dump constructor runs. Only
    /// invoked by `toasty` itself.
    #[command(name = "__load-cdylib", hide = true)]
    LoadCdylib(load_cdylib::LoadCdylibCommand),
}
