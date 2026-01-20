mod config;
mod history_file;
mod snapshot_file;

use std::fs;

pub use config::*;
pub use history_file::*;
pub use snapshot_file::*;

use crate::Config;
use anyhow::Result;
use clap::Parser;
use toasty::{
    Db,
    schema::db::{RenameHints, Schema, SchemaDiff},
};

#[derive(Parser, Debug)]
pub struct MigrationCommand {
    #[command(subcommand)]
    subcommand: MigrationSubcommand,
}

#[derive(Parser, Debug)]
enum MigrationSubcommand {
    /// Generate a new migration based on schema changes
    Generate(GenerateCommand),

    /// Print the current schema snapshot file
    Snapshot(SnapshotCommand),
}

#[derive(Parser, Debug)]
pub struct GenerateCommand {
    /// Name for the migration
    #[arg(short, long)]
    name: Option<String>,
}

#[derive(Parser, Debug)]
pub struct SnapshotCommand {
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
            Self::Snapshot(cmd) => cmd.run(db, config),
        }
    }
}

impl GenerateCommand {
    fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!("Generating migration...");
        let history_path = config.migration.get_history_file_path();

        fs::create_dir_all(config.migration.get_migrations_dir())?;
        fs::create_dir_all(config.migration.get_snapshots_dir())?;
        fs::create_dir_all(history_path.parent().unwrap())?;

        let mut history = HistoryFile::load_or_default(&history_path)?;

        let previous_snapshot = history
            .migrations()
            .last()
            .map(|f| {
                SnapshotFile::load(config.migration.get_snapshots_dir().join(&f.snapshot_name))
            })
            .transpose()?;
        let previous_schema = previous_snapshot
            .map(|snapshot| snapshot.schema)
            .unwrap_or_else(Schema::default);

        let schema = toasty::schema::db::Schema::clone(&db.schema().db);
        let snapshot = SnapshotFile::new(schema.clone());
        let snapshot_name = format!("{:04}_snapshot.toml", history.migrations().len());
        let snapshot_path = config.migration.get_snapshots_dir().join(&snapshot_name);

        let migration_name = format!(
            "{:04}_{}.sql",
            history.migrations().len(),
            self.name.as_deref().unwrap_or("migration")
        );
        let migration_path = config.migration.get_migrations_dir().join(&migration_name);

        let diff = SchemaDiff::from(&previous_schema, &schema, &RenameHints::default());

        history.add_migration(HistoryFileMigration {
            name: migration_name,
            snapshot_name,
            checksum: None,
        });

        eprintln!("{:?}", snapshot_path);
        snapshot.save(&snapshot_path)?;
        history.save(&history_path)?;

        Ok(())
    }
}

impl SnapshotCommand {
    fn run(self, db: &Db, _config: &Config) -> Result<()> {
        let snapshot_file = SnapshotFile::new(toasty::schema::db::Schema::clone(&db.schema().db));
        println!("{}", snapshot_file);
        Ok(())
    }
}
