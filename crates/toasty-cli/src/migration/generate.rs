use crate::Config;
use super::{HistoryFile, HistoryFileMigration, SnapshotFile};
use anyhow::Result;
use clap::Parser;
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
        println!("Generating migration...");
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
        let snapshot = SnapshotFile::new(schema.clone());
        let snapshot_name = format!("{:04}_snapshot.toml", history.migrations().len());
        let snapshot_path = config.migration.get_snapshots_dir().join(&snapshot_name);

        let migration_name = format!(
            "{:04}_{}.sql",
            history.migrations().len(),
            self.name.as_deref().unwrap_or("migration")
        );
        let _migration_path = config.migration.get_migrations_dir().join(&migration_name);

        let _diff = SchemaDiff::from(&previous_schema, &schema, &RenameHints::default());

        history.add_migration(HistoryFileMigration {
            name: migration_name,
            snapshot_name,
            checksum: None,
        });

        eprintln!("{:?}", snapshot_path);
        snapshot.save(&snapshot_path)?;
        history.save(&history_path)?;

        Ok(())
    }
}
