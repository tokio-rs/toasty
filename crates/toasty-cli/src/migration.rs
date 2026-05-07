mod apply;
mod config;
mod drop;
mod generate;
mod history_file;
mod reset;
mod snapshot_file;

pub use apply::*;
pub use config::*;
pub use drop::*;
pub use generate::*;
pub use history_file::*;
pub use reset::*;
pub use snapshot_file::*;

use clap::Parser;

/// Top-level `migration` subcommand.
///
/// Groups all migration-related subcommands: apply, generate, drop, and reset.
/// This struct is used by clap to parse `toasty migration <sub>`.
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

    /// Drop a migration from the history
    Drop(DropCommand),

    /// Reset the database (drop all tables) and optionally re-apply migrations
    Reset(ResetCommand),
}
