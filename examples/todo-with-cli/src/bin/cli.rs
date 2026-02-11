use example_todo_with_cli::create_db;
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration from Toasty.toml
    let config = Config::load()?;

    // Create the database instance with our schema
    let db = create_db().await?;

    // Create the CLI with our database and config
    let cli = ToastyCli::with_config(db, config);

    // Parse and run CLI commands
    cli.parse_and_run().await?;

    Ok(())
}
