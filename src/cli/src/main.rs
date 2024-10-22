use toasty_cli::{gen, init};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Gen {
        #[clap(short, long)]
        schema: String,
        target: String,
    },
    Init,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Gen { schema, target } => gen::exec(&schema, &target),
        Command::Init => init::exec(),
    }
}
