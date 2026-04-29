use anyhow::Result;
use toasty_cli::ToastyCli;

#[tokio::main]
async fn main() -> Result<()> {
    ToastyCli::standalone()?.parse_and_run().await
}
