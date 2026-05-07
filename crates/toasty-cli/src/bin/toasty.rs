use anyhow::Result;
use toasty_cli::ToastyCli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    ToastyCli::new()?.parse_and_run().await
}
