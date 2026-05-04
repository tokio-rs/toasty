use crate::{Config, theme::dialoguer_theme};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Select;
use toasty::Db;
use toasty::migrate::{DropTarget, HistoryFile};

/// Removes a migration from the history and deletes its files on disk.
///
/// The migration to drop can be specified by `--name`, by `--latest`, or by
/// interactive selection when neither flag is provided. Dropping a migration
/// removes its SQL file, its snapshot file, and its entry in the history
/// file. It does **not** undo any changes already applied to the database.
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
        let history = HistoryFile::load_or_default(config.migration.history_file_path())?;

        if history.migrations().is_empty() {
            eprintln!("{}", style("No migrations found in history").red().bold());
            anyhow::bail!("No migrations found in history");
        }

        let target = if self.latest {
            DropTarget::Latest
        } else if let Some(name) = self.name.as_deref() {
            DropTarget::Name(name)
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

            let index = Select::with_theme(&dialoguer_theme())
                .with_prompt("  Select migration to drop")
                .items(&migration_display)
                .default(migration_display.len() - 1)
                .interact()?;
            DropTarget::Index(index)
        };

        println!();

        let dropped = toasty::migrate::drop_migration(&config.migration, target)?;

        if dropped.migration_file_deleted {
            println!(
                "  {} {}",
                style("✓").green().bold(),
                style(format!("Deleted migration: {}", dropped.migration_name)).dim()
            );
        } else {
            println!(
                "  {} {}",
                style("⚠").yellow().bold(),
                style(format!(
                    "Migration file not found: {}",
                    dropped.migration_name
                ))
                .yellow()
                .dim()
            );
        }

        if dropped.snapshot_file_deleted {
            println!(
                "  {} {}",
                style("✓").green().bold(),
                style(format!("Deleted snapshot: {}", dropped.snapshot_name)).dim()
            );
        } else {
            println!(
                "  {} {}",
                style("⚠").yellow().bold(),
                style(format!(
                    "Snapshot file not found: {}",
                    dropped.snapshot_name
                ))
                .yellow()
                .dim()
            );
        }

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
                dropped.migration_name
            ))
            .green()
            .bold()
        );
        println!();

        Ok(())
    }
}
