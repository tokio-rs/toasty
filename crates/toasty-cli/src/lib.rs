//! The standalone `toasty` command-line tool.
//!
//! Installed with `cargo install toasty-cli` and run from any Cargo package
//! that uses Toasty. The CLI extracts the package's resolved schema by
//! building the package's existing bin target (or its lib as a `cdylib`) and
//! running the artifact with `TOASTY_DUMP_SCHEMA` set, which triggers a
//! constructor inside `toasty` that dumps the schema and exits. See
//! `docs/dev/design/standalone-cli.md` in the Toasty repository.
//!
//! This crate is a binary; the library exists for the binary and the crate's
//! integration tests. Its API is not stable.

mod cargo;
mod config;
mod extract;
mod flavor;
mod load_cdylib;
mod migrate;
mod project;
mod theme;
mod utility;

pub use config::Config;
pub use flavor::Flavor;
pub use migrate::{MigrationConfig, MigrationPrefixStyle};
pub use project::Project;

use anyhow::Result;
use clap::Parser;

/// Parses `std::env::args` and runs the matching subcommand.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    run_parsed(cli).await
}

/// Parses from an explicit argument list and runs the matching subcommand.
/// Useful for testing.
pub async fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run_parsed(cli).await
}

async fn run_parsed(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Migrate(cmd) => {
            let project = Project::locate(cli.package.as_deref())?;
            cmd.run(&project).await
        }
        Command::LoadCdylib(cmd) => cmd.run(),
    }
}

#[derive(Parser, Debug)]
#[command(name = "toasty")]
#[command(about = "Toasty schema management and migrations")]
#[command(version)]
struct Cli {
    /// Package to operate on, when the workspace has more than one
    #[arg(short, long, global = true, value_name = "PKG")]
    package: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    /// Database migration commands
    #[command(alias = "migration")]
    Migrate(migrate::MigrateCommand),

    /// Internal: load a cdylib so its schema-dump constructor runs.
    ///
    /// Only invoked by `toasty` itself while extracting a schema from a
    /// lib-only package.
    #[command(name = "__load-cdylib", hide = true)]
    LoadCdylib(load_cdylib::LoadCdylibCommand),
}
