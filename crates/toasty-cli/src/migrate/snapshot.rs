use crate::{Flavor, Project, extract};
use anyhow::Result;
use clap::Parser;
use console::style;
use toasty::migration::Snapshot;

/// Prints the current schema as a TOML snapshot to stdout.
///
/// Compiles the target package, extracts its schema, and formats it as a
/// [`Snapshot`]. Table headers, key-value pairs, and whitespace are
/// syntax-highlighted for terminal display.
#[derive(Parser, Debug)]
pub struct SnapshotCommand {
    /// Database flavor to lower the schema for
    #[arg(long, value_enum)]
    flavor: Option<Flavor>,

    /// Bin target to extract the schema from, when the package has several
    #[arg(long)]
    bin: Option<String>,
}

impl SnapshotCommand {
    pub(crate) fn run(self, project: &Project) -> Result<()> {
        // Headers and progress go to stderr; stdout carries the TOML alone, so
        // `toasty migrate snapshot > schema.toml` yields a parseable file.
        eprintln!();
        eprintln!(
            "  {}",
            style("Current Schema Snapshot").cyan().bold().underlined()
        );
        eprintln!();

        let flavor = project.flavor(self.flavor)?;
        let schema = extract::extract_schema(project, flavor, self.bin.as_deref())?;
        let snapshot = Snapshot::new(schema);

        // Table headers are highlighted for terminal display. `console`
        // suppresses the escapes when stdout is not a terminal, and nothing
        // here reflows the line, so a redirected snapshot is byte-for-byte
        // the TOML that `Snapshot` produced.
        for line in snapshot.to_toml_string()?.lines() {
            if line.starts_with('[') {
                println!("{}", style(line).yellow().bold());
            } else {
                println!("{line}");
            }
        }

        Ok(())
    }
}
