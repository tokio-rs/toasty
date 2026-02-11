use super::SnapshotFile;
use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;
use toasty::Db;

#[derive(Parser, Debug)]
pub struct SnapshotCommand {
    // Future options can be added here
}

impl SnapshotCommand {
    pub(crate) fn run(self, db: &Db, _config: &Config) -> Result<()> {
        println!();
        println!(
            "  {}",
            style("Current Schema Snapshot").cyan().bold().underlined()
        );
        println!();

        let snapshot_file = SnapshotFile::new(toasty::schema::db::Schema::clone(&db.schema().db));

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
