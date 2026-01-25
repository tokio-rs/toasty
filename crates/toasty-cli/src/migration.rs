mod apply;
mod config;
mod drop;
mod generate;
mod history_file;
mod snapshot;
mod snapshot_file;

pub use apply::*;
pub use config::*;
pub use drop::*;
pub use generate::*;
pub use history_file::*;
pub use snapshot::*;
pub use snapshot_file::*;

use crate::Config;
use anyhow::Result;
use clap::Parser;
use toasty::Db;

#[derive(Parser, Debug)]
pub struct MigrationCommand {
    #[command(subcommand)]
    subcommand: MigrationSubcommand,
}

#[derive(Parser, Debug)]
enum MigrationSubcommand {
    /// Apply pending migrations to the database
    Apply(ApplyCommand),

    /// Generate a new migration based on schema changes
    Generate(GenerateCommand),

    /// Print the current schema snapshot file
    Snapshot(SnapshotCommand),

    /// Drop a migration from the history
    Drop(DropCommand),
}

impl MigrationCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        self.subcommand.run(db, config).await
    }
}

impl MigrationSubcommand {
    async fn run(self, db: &Db, config: &Config) -> Result<()> {
        match self {
            Self::Apply(cmd) => cmd.run(db, config).await,
            Self::Generate(cmd) => cmd.run(db, config),
            Self::Snapshot(cmd) => cmd.run(db, config),
            Self::Drop(cmd) => cmd.run(db, config),
        }
    }
}
