use super::apply_migrations;
use crate::theme::dialoguer_theme;
use crate::{Config, ConnectArgs};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Confirm;
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

    #[command(flatten)]
    connect: ConnectArgs,
}

impl ResetCommand {
    pub(crate) async fn run(self, config: &Config) -> Result<()> {
        let driver = self.connect.driver().await?;
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

        if !self.skip_migrations {
            apply_migrations(&driver, config).await?;
        }

        Ok(())
    }
}
