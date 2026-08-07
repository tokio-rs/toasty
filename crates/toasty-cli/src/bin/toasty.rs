use console::style;

#[tokio::main]
async fn main() {
    if let Err(err) = toasty_cli::run().await {
        eprintln!("{} {err:#}", style("error:").red().bold());
        std::process::exit(1);
    }
}
