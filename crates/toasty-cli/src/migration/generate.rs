use super::{HistoryFile, HistoryFileMigration, SnapshotFile};
use crate::Config;
use anyhow::Result;
use clap::Parser;
use console::style;
use std::fs;
use toasty::{
    Db,
    schema::db::{RenameHints, Schema, SchemaDiff},
};

#[derive(Parser, Debug)]
pub struct GenerateCommand {
    /// Name for the migration
    #[arg(short, long)]
    name: Option<String>,
}

impl GenerateCommand {
    pub(crate) fn run(self, db: &Db, config: &Config) -> Result<()> {
        println!();
        println!(
            "  {}",
            style("Generate Migration").cyan().bold().underlined()
        );
        println!();

        let history_path = config.migration.get_history_file_path();

        fs::create_dir_all(config.migration.get_migrations_dir())?;
        fs::create_dir_all(config.migration.get_snapshots_dir())?;
        fs::create_dir_all(history_path.parent().unwrap())?;

        let mut history = HistoryFile::load_or_default(&history_path)?;

        let previous_snapshot = history
            .migrations()
            .last()
            .map(|f| {
                SnapshotFile::load(config.migration.get_snapshots_dir().join(&f.snapshot_name))
            })
            .transpose()?;
        let previous_schema = previous_snapshot
            .map(|snapshot| snapshot.schema)
            .unwrap_or_else(Schema::default);

        let schema = toasty::schema::db::Schema::clone(&db.schema().db);

        let rename_hints = RenameHints::default();
        let diff = SchemaDiff::from(&previous_schema, &schema, &rename_hints);

        if diff.is_empty() {
            println!(
                "  {}",
                style("The current schema matches the previous snapshot. No migration needed.")
                    .magenta()
                    .dim()
            );
            println!();
            return Ok(());
        }

        let snapshot = SnapshotFile::new(schema.clone());
        let migration_number = history.next_migration_number();
        let snapshot_name = format!("{:04}_snapshot.toml", migration_number);
        let snapshot_path = config.migration.get_snapshots_dir().join(&snapshot_name);

        let migration_name = format!(
            "{:04}_{}.sql",
            migration_number,
            self.name.as_deref().unwrap_or("migration")
        );
        let _migration_path = config.migration.get_migrations_dir().join(&migration_name);

        history.add_migration(HistoryFileMigration {
            name: migration_name.clone(),
            snapshot_name: snapshot_name.clone(),
            checksum: None,
        });

        snapshot.save(&snapshot_path)?;
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Created snapshot: {}", snapshot_name)).dim()
        );

        history.save(&history_path)?;
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style("Updated migration history").dim()
        );

        println!();
        println!(
            "  {}",
            style(format!(
                "Migration '{}' generated successfully",
                migration_name
            ))
            .green()
            .bold()
        );
        println!();

        Ok(())
    }
}
