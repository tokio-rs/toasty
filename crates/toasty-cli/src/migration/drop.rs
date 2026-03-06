use super::HistoryFile;
use crate::{Config, theme::dialoguer_theme};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Select;
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
            eprintln!("{}", style("No migrations found in history").red().bold());
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
            // Interactive picker with fancy theme
            println!();
            println!("  {}", style("Drop Migration").cyan().bold().underlined());
            println!();

            let migration_display: Vec<String> = history
                .migrations()
                .iter()
                .map(|m| format!("  {}", m.name))
                .collect();

            Select::with_theme(&dialoguer_theme())
                .with_prompt("  Select migration to drop")
                .items(&migration_display)
                .default(migration_display.len() - 1)
                .interact()?
        };

        println!();

        let migration = &history.migrations()[migration_index];
        let migration_name = migration.name.clone();
        let snapshot_name = migration.snapshot_name.clone();

        // Delete migration file
        let migration_path = config.migration.get_migrations_dir().join(&migration_name);
        if migration_path.exists() {
            fs::remove_file(&migration_path)?;
            println!(
                "  {} {}",
                style("✓").green().bold(),
                style(format!("Deleted migration: {}", migration_name)).dim()
            );
        } else {
            println!(
                "  {} {}",
                style("⚠").yellow().bold(),
                style(format!("Migration file not found: {}", migration_name))
                    .yellow()
                    .dim()
            );
        }

        // Delete snapshot file
        let snapshot_path = config.migration.get_snapshots_dir().join(&snapshot_name);
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path)?;
            println!(
                "  {} {}",
                style("✓").green().bold(),
                style(format!("Deleted snapshot: {}", snapshot_name)).dim()
            );
        } else {
            println!(
                "  {} {}",
                style("⚠").yellow().bold(),
                style(format!("Snapshot file not found: {}", snapshot_name))
                    .yellow()
                    .dim()
            );
        }

        // Remove from history
        history.remove_migration(migration_index);
        history.save(&history_path)?;

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style("Updated migration history").dim()
        );
        println!();
        println!(
            "  {} {}",
            style("").magenta(),
            style(format!(
                "Migration '{}' successfully dropped",
                migration_name
            ))
            .green()
            .bold()
        );
        println!();

        Ok(())
    }
}
