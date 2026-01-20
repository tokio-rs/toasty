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

    /// Drop a migration from the history
    Drop(DropCommand),
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

#[derive(Parser, Debug)]
pub struct DropCommand {
    /// Name of the migration to drop (if not provided, will prompt)
    #[arg(short, long)]
    name: Option<String>,

    /// Drop the latest migration
    #[arg(short, long)]
    latest: bool,
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
            Self::Drop(cmd) => cmd.run(db, config),
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

impl DropCommand {
    fn run(self, _db: &Db, config: &Config) -> Result<()> {
        let history_path = config.migration.get_history_file_path();
        let mut history = HistoryFile::load_or_default(&history_path)?;

        if history.migrations().is_empty() {
            anyhow::bail!("No migrations found in history");
        }

        // Determine which migration to drop
        let migration_index = if self.latest {
            // Drop the latest migration
            history.migrations().len() - 1
        } else if let Some(name) = &self.name {
            // Find migration by name
            history
                .migrations()
                .iter()
                .position(|m| m.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Migration '{}' not found", name))?
        } else {
            // Interactive picker
            use dialoguer::Select;

            let migration_names: Vec<String> = history
                .migrations()
                .iter()
                .map(|m| m.name.clone())
                .collect();

            Select::new()
                .with_prompt("Select migration to drop")
                .items(&migration_names)
                .default(migration_names.len() - 1)
                .interact()?
        };

        let migration = &history.migrations()[migration_index];
        let migration_name = migration.name.clone();
        let snapshot_name = migration.snapshot_name.clone();

        // Delete migration file
        let migration_path = config.migration.get_migrations_dir().join(&migration_name);
        if migration_path.exists() {
            fs::remove_file(&migration_path)?;
            println!("Deleted migration file: {}", migration_name);
        } else {
            eprintln!("Warning: Migration file not found: {}", migration_name);
        }

        // Delete snapshot file
        let snapshot_path = config.migration.get_snapshots_dir().join(&snapshot_name);
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path)?;
            println!("Deleted snapshot file: {}", snapshot_name);
        } else {
            eprintln!("Warning: Snapshot file not found: {}", snapshot_name);
        }

        // Remove from history
        history.remove_migration(migration_index);
        history.save(&history_path)?;

        println!("Removed migration '{}' from history", migration_name);

        Ok(())
    }
}
