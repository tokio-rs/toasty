use toasty_cli::{gen, init};

use anyhow::Result;
use clap::{
    builder::styling::{AnsiColor, Style, Styles},
    Parser, Subcommand,
};

pub const HEADER: Style = AnsiColor::Green.on_default().bold();
pub const USAGE: Style = AnsiColor::Green.on_default().bold();
pub const LITERAL: Style = AnsiColor::Cyan.on_default().bold();
pub const PLACEHOLDER: Style = AnsiColor::Cyan.on_default();
pub const ERROR: Style = AnsiColor::Red.on_default().bold();
pub const VALID: Style = AnsiColor::Cyan.on_default().bold();
pub const INVALID: Style = AnsiColor::Yellow.on_default().bold();

pub const HELP_STYLES: Styles = Styles::styled()
    .header(HEADER)
    .usage(USAGE)
    .literal(LITERAL)
    .placeholder(PLACEHOLDER)
    .error(ERROR)
    .valid(VALID)
    .invalid(INVALID);

#[derive(Parser, Debug)]
#[clap(version, about, styles = HELP_STYLES)]
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
