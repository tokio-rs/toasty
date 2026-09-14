use console::style;

fn main() {
    if let Err(err) = toasty_cli::run() {
        eprintln!("{} {err:#}", style("error:").red().bold());
        std::process::exit(1);
    }
}
