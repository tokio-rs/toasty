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
        schema: Option<String>,
        target: String,
    },
    Init {
        root_dir: Option<String>,
    },
    InitExample {
        name: Option<String>,
    },
    GenExample {
        target: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Gen { schema, target } => gen::exec(schema, target),
        Command::Init { root_dir } => init::exec(),
        Command::InitExample { name } => init::exec_example(name),
        Command::GenExample { target } => gen::exec_example(target),
    }
}
