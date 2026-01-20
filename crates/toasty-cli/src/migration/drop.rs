use crate::Config;
use super::HistoryFile;
use anyhow::Result;
use clap::Parser;
use std::fs;
use toasty::Db;

#[derive(Parser, Debug)]
pub struct DropCommand {
    /// Name of the migration to drop (if not provided, will prompt)
    #[arg(short, long)]
    name: Option<String>,

    /// Drop the latest migration
    #[arg(short, long)]
    latest: bool,
}

impl DropCommand {
    pub(crate) fn run(self, _db: &Db, config: &Config) -> Result<()> {
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
