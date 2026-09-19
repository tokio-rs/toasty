use super::apply::apply_migrations;
use crate::Project;
use crate::theme::dialoguer_theme;
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Confirm;

/// Drops all tables in the database, then optionally re-applies migrations.
///
/// Prompts for confirmation before proceeding. After the reset, all
/// migrations from the history file are re-applied unless `--skip-migrations`
/// is passed.
#[derive(Parser, Debug)]
pub struct ResetCommand {
    /// Database URL to reset
    #[arg(long, env = "DATABASE_URL")]
    url: String,

    /// Skip applying migrations after reset
    #[arg(long)]
    skip_migrations: bool,

    /// Skip the confirmation prompt
    #[arg(short = 'y', long)]
    yes: bool,
}

impl ResetCommand {
    pub(crate) async fn run(self, project: &Project) -> Result<()> {
        println!();
        println!("  {}", style("Reset Database").cyan().bold().underlined());
        println!();

        let db = crate::utility::connect(&self.url).await?;
        println!(
            "  {}",
            style(format!(
                "Connected to {}",
                crate::utility::redact_url_password(&self.url)
            ))
            .dim()
        );
        println!();

        if !self.yes {
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
        }

        println!();
        println!("  {} Resetting database...", style("→").cyan());

        db.reset_db().await?;

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style("Database reset successfully").dim()
        );
        println!();

        if !self.skip_migrations {
            apply_migrations(&db, project).await?;
        }

        Ok(())
    }
}
