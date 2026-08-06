use super::apply::apply_migrations;
use crate::Config;
use crate::theme::dialoguer_theme;
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use toasty::Db;
use toasty::db::Driver;

/// Drops all tables in the database, then optionally re-applies migrations.
///
/// Prompts for confirmation before proceeding. After the reset, all
/// migrations from the history file are re-applied unless `--skip-migrations`
/// is passed.
#[derive(Parser, Debug)]
pub struct ResetCommand {
    /// Skip applying migrations after reset
    #[arg(long)]
    skip_migrations: bool,
}

impl ResetCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        run_reset(db.driver(), config, self.skip_migrations).await
    }
}

/// Drops every table on `driver` and optionally re-applies the migrations.
///
/// Driver-based for the same reason as [`run_apply`](super::apply::run_apply):
/// the models play no part.
pub(crate) async fn run_reset(
    driver: &dyn Driver,
    config: &Config,
    skip_migrations: bool,
) -> Result<()> {
    {
        println!();
        println!("  {}", style("Reset Database").cyan().bold().underlined());
        println!();
        println!(
            "  {}",
            style(format!(
                "Connected to {}",
                crate::utility::redact_url_password(&driver.url())
            ))
            .dim()
        );
        println!();

        let theme = {
            let mut t = dialoguer_theme();
            t.success_prefix = style(" ".to_string());
            t.prompt_prefix = style(" ".to_string());
            t.prompt_style = console::Style::new().red().bold();
            t
        };

        let confirmed = Confirm::with_theme(&theme)
            .with_prompt("This will drop all tables and data. Are you sure?")
            .default(false)
            .interact()?;

        if !confirmed {
            println!();
            println!("  {}", style("Aborted.").dim());
            println!();
            return Ok(());
        }

        println!();
        println!("  {} Resetting database...", style("→").cyan());

        driver.reset_db().await?;

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style("Database reset successfully").dim()
        );
        println!();

        if !skip_migrations {
            apply_migrations(driver, config).await?;
        }

        Ok(())
    }
}
