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
        println!();
        println!(
            "  {}",
            style("Current Schema Snapshot").cyan().bold().underlined()
        );
        println!();

        let flavor = project.flavor(self.flavor)?;
        let schema = extract::extract_schema(project, flavor, self.bin.as_deref())?;
        let snapshot = Snapshot::new(schema);

        // Print the snapshot with nice formatting
        let snapshot_str = snapshot.to_toml_string()?;
        for line in snapshot_str.lines() {
            if line.starts_with('[') {
                println!("  {}", style(line).yellow().bold());
            } else if line.contains('=') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    println!(
                        "  {}{} {}",
                        style(parts[0]).cyan(),
                        style("=").dim(),
                        style(parts[1]).green()
                    );
                } else {
                    println!("  {}", style(line).dim());
                }
            } else if line.trim().is_empty() {
                println!();
            } else {
                println!("  {}", style(line).dim());
            }
        }

        println!();
        Ok(())
    }
}
