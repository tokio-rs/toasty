mod apply;
mod config;
mod drop;
mod generate;
mod reset;
mod snapshot;

pub use config::{MigrationConfig, MigrationPrefixStyle};

use crate::Project;
use anyhow::Result;
use clap::Parser;

/// Top-level `migrate` subcommand.
///
/// Groups all migration-related subcommands: generate, apply, reset, drop,
/// and snapshot. This struct is used by clap to parse `toasty migrate <sub>`.
#[derive(Parser, Debug)]
pub struct MigrateCommand {
    #[command(subcommand)]
    subcommand: MigrateSubcommand,
}

#[derive(Parser, Debug)]
enum MigrateSubcommand {
    /// Generate a new migration from schema changes
    Generate(generate::GenerateCommand),

    /// Apply pending migrations to the database
    Apply(apply::ApplyCommand),

    /// Reset the database (drop all tables) and re-apply migrations
    Reset(reset::ResetCommand),

    /// Drop a migration from the history
    Drop(drop::DropCommand),

    /// Print the current schema snapshot
    Snapshot(snapshot::SnapshotCommand),
}

impl MigrateCommand {
    pub(crate) async fn run(self, project: &Project) -> Result<()> {
        match self.subcommand {
            MigrateSubcommand::Generate(cmd) => cmd.run(project),
            MigrateSubcommand::Apply(cmd) => cmd.run(project).await,
            MigrateSubcommand::Reset(cmd) => cmd.run(project).await,
            MigrateSubcommand::Drop(cmd) => cmd.run(project),
            MigrateSubcommand::Snapshot(cmd) => cmd.run(project),
        }
    }
}
