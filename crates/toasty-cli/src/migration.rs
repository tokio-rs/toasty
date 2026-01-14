mod config;
mod snapshot_file;

pub use config::*;
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
    /// Generate a new migration based on schema changes
    Generate(GenerateCommand),

    /// Print the current schema lock file
    Lock(LockCommand),
}

#[derive(Parser, Debug)]
pub struct GenerateCommand {
    // Future options can be added here, e.g.:
    // /// Name for the migration
    // #[arg(short, long)]
    // name: Option<String>,
}

#[derive(Parser, Debug)]
pub struct LockCommand {
    // Future options can be added here
}

impl MigrationCommand {
    pub(crate) fn run(self, db: &Db, config: &Config) -> Result<()> {
        self.subcommand.run(db, config)
    }
}

impl MigrationSubcommand {
    fn run(self, db: &Db, config: &Config) -> Result<()> {
        match self {
            Self::Generate(cmd) => cmd.run(db, config),
            Self::Lock(cmd) => cmd.run(db, config),
        }
    }
}

impl GenerateCommand {
    fn run(self, _db: &Db, config: &Config) -> Result<()> {
        // TODO: Implement migration generation logic
        println!("Generating migration...");
        println!("Migrations path: {:?}", config.migration.migrations_path);
        println!("Prefix style: {:?}", config.migration.prefix_style);
        println!("Migration generation is not yet implemented");
        Ok(())
    }
}

impl LockCommand {
    fn run(self, db: &Db, _config: &Config) -> Result<()> {
        let lock_file = SnapshotFile::new(toasty::schema::db::Schema::clone(&db.schema().db));
        println!("{}", lock_file);
        Ok(())
    }
}
