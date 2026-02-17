use super::apply_migrations;
use crate::Config;
use crate::theme::dialoguer_theme;
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use toasty::Db;

#[derive(Parser, Debug)]
pub struct ResetCommand {
    /// Skip applying migrations after reset
    #[arg(long)]
    skip_migrations: bool,
}

impl ResetCommand {
    pub(crate) async fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!();
        println!("  {}", style("Reset Database").cyan().bold().underlined());
        println!();
        println!(
            "  {}",
            style(format!("Connected to {}", db.driver().url())).dim()
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

        db.reset_db().await?;

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style("Database reset successfully").dim()
        );
        println!();

        if !self.skip_migrations {
            apply_migrations(db, config).await?;
        }

        Ok(())
    }
}
