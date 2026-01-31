use super::HistoryFile;
use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;
use std::collections::HashSet;
use std::fs;
use toasty::Db;
use toasty::schema::db::Migration;

#[derive(Parser, Debug)]
pub struct ApplyCommand {}

impl ApplyCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!();
        println!("  {}", style("Apply Migrations").cyan().bold().underlined());
        println!();

        let history_path = config.migration.get_history_file_path();

        // Load migration history
        let history = HistoryFile::load_or_default(&history_path)?;

        if history.migrations().is_empty() {
            println!(
                "  {}",
                style("No migrations found in history file.")
                    .magenta()
                    .dim()
            );
            println!();
            return Ok(());
        }

        // Get a connection to check which migrations have been applied
        let mut conn = db.driver().connect().await?;

        // Get list of already applied migrations
        let applied_migrations = conn.applied_migrations().await?;
        let applied_ids: HashSet<u64> = applied_migrations.iter().map(|m| m.id()).collect();

        // Find migrations that haven't been applied yet
        let pending_migrations: Vec<_> = history
            .migrations()
            .iter()
            .filter(|m| !applied_ids.contains(&m.id))
            .collect();

        if pending_migrations.is_empty() {
            println!(
                "  {}",
                style("All migrations are already applied. Database is up to date.")
                    .green()
                    .dim()
            );
            println!();
            return Ok(());
        }

        let pending_count = pending_migrations.len();
        println!(
            "  {} Found {} pending migration(s) to apply",
            style("→").cyan(),
            pending_count
        );
        println!();

        // Apply each pending migration
        for migration_entry in &pending_migrations {
            let migration_path = config
                .migration
                .get_migrations_dir()
                .join(&migration_entry.name);

            println!(
                "  {} Applying migration: {}",
                style("→").cyan(),
                style(&migration_entry.name).bold()
            );

            // Load the migration SQL file
            let sql = fs::read_to_string(&migration_path)?;
            let migration = Migration::new_sql(sql);

            // Apply the migration
            conn.apply_migration(migration_entry.id, migration_entry.name.clone(), &migration)
                .await?;

            println!(
                "  {} {}",
                style("✓").green().bold(),
                style(format!("Applied: {}", migration_entry.name)).dim()
            );
        }

        println!();
        println!(
            "  {}",
            style(format!(
                "Successfully applied {} migration(s)",
                pending_count
            ))
            .green()
            .bold()
        );
        println!();

        Ok(())
    }
}
