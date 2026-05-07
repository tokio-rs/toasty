use super::{HistoryFile, SnapshotFile};
use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;

/// Prints the most recently generated schema snapshot to stdout.
///
/// Reads the latest entry from the migration history and pretty-prints the
/// corresponding `*_snapshot.toml` file. Does not connect to a database or
/// invoke the dumper.
#[derive(Parser, Debug)]
pub struct SnapshotCommand {}

impl SnapshotCommand {
    pub(crate) fn run(self, config: &Config) -> Result<()> {
        let history_path = config.migration.get_history_file_path();
        let history = HistoryFile::load_or_default(&history_path)?;

        let Some(latest) = history.migrations().last() else {
            println!();
            println!(
                "  {}",
                style("No migrations have been generated yet.")
                    .magenta()
                    .dim()
            );
            println!();
            return Ok(());
        };

        let snapshot_path = config
            .migration
            .get_snapshots_dir()
            .join(&latest.snapshot_name);
        let snapshot_file = SnapshotFile::load(&snapshot_path)?;

        println!();
        println!(
            "  {}",
            style(format!("Schema Snapshot: {}", latest.snapshot_name))
                .cyan()
                .bold()
                .underlined()
        );
        println!();

        // Print the snapshot with nice formatting
        let snapshot_str = snapshot_file.to_string();
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
