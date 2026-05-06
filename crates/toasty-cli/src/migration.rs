mod apply;
mod config;
mod drop;
mod generate;
mod history_file;
mod reset;
mod snapshot;
mod snapshot_file;

pub use apply::*;
pub use config::*;
pub use drop::*;
pub use generate::*;
pub use history_file::*;
pub use reset::*;
pub use snapshot::*;
pub use snapshot_file::*;

use crate::Config;
use anyhow::Result;
use clap::Parser;
use toasty::Db;

/// Top-level `migration` subcommand.
///
/// Groups all migration-related subcommands: apply, generate, snapshot, drop,
/// and reset. This struct is used by clap to parse `toasty migration <sub>`.
#[derive(Parser, Debug)]
pub struct MigrationCommand {
    #[command(subcommand)]
    pub(crate) subcommand: MigrationSubcommand,
}

#[derive(Parser, Debug)]
pub(crate) enum MigrationSubcommand {
    /// Apply pending migrations to the database
    Apply(ApplyCommand),

    /// Generate a new migration based on schema changes
    Generate(GenerateCommand),

    /// Print the current schema snapshot file
    Snapshot(SnapshotCommand),

    /// Drop a migration from the history
    Drop(DropCommand),

    /// Reset the database (drop all tables) and optionally re-apply migrations
    Reset(ResetCommand),
}

impl MigrationSubcommand {
    /// Run a non-generate subcommand against a connected `Db`. Generate is
    /// dispatched directly by [`ToastyCli::run`](crate::ToastyCli) via
    /// [`GenerateCommand::run`], without a connection.
    pub(crate) async fn run_with_db(self, db: &Db, config: &Config) -> Result<()> {
        match self {
            Self::Apply(cmd) => cmd.run(db, config).await,
            Self::Generate(_) => {
                unreachable!("Generate is dispatched offline, not via run_with_db")
            }
            Self::Snapshot(cmd) => cmd.run(db, config),
            Self::Drop(cmd) => cmd.run(db, config),
            Self::Reset(cmd) => cmd.run(db, config).await,
        }
    }
}
