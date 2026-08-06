//! The standalone `toasty` binary. See [`toasty_cli::standalone`].

fn main() {
    if let Err(err) = toasty_cli::standalone::run() {
        eprintln!("{} {err:#}", console::style("error:").red().bold());
        std::process::exit(1);
    }
}
