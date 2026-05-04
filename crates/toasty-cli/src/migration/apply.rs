use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;
use toasty::Db;

/// Applies pending migrations to the database.
///
/// Reads the migration history file to determine which migrations exist, then
/// queries the database for already-applied migrations. Any migration present
/// in the history but not yet applied is executed in order.
///
/// If no pending migrations are found, the command prints a message and exits
/// without modifying the database.
#[derive(Parser, Debug)]
pub struct ApplyCommand {}

impl ApplyCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!();
        println!("  {}", style("Apply Migrations").cyan().bold().underlined());
        println!();
        println!(
            "  {}",
            style(format!(
                "Connected to {}",
                crate::utility::redact_url_password(&db.driver().url())
            ))
            .dim()
        );
        println!();

        run_apply(db, &config.migration).await
    }
}

pub(crate) async fn run_apply(db: &Db, config: &toasty::migrate::Config) -> Result<()> {
    let pending = toasty::migrate::pending(db, config).await?;

    if pending.is_empty() {
        println!(
            "  {}",
            style("All migrations are already applied. Database is up to date.")
                .green()
                .dim()
        );
        println!();
        return Ok(());
    }

    let pending_count = pending.len();
    println!(
        "  {} Found {} pending migration(s) to apply",
        style("→").cyan(),
        pending_count
    );
    println!();

    let applied = toasty::migrate::apply(db, config).await?;

    for entry in &applied {
        println!(
            "  {} Applying migration: {}",
            style("→").cyan(),
            style(&entry.name).bold()
        );
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Applied: {}", entry.name)).dim()
        );
    }

    println!();
    println!(
        "  {}",
        style(format!(
            "Successfully applied {} migration(s)",
            applied.len()
        ))
        .green()
        .bold()
    );
    println!();

    Ok(())
}
