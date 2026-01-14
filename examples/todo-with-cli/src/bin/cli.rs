use example_todo_with_cli::create_db;
use toasty_cli::ToastyCli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create the database instance with our schema
    let db = create_db().await?;

    // Create the CLI with our database
    let cli = ToastyCli::new(db);

    // Parse and run CLI commands
    cli.parse_and_run()?;

    Ok(())
}
