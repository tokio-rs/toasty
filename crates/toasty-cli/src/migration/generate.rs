use super::{HistoryFile, HistoryFileMigration, SnapshotFile};
use crate::{Config, theme::dialoguer_theme};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Select;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fs;
use toasty::{
    Db,
    schema::db::{
        ColumnId, ColumnsDiffItem, IndexId, IndicesDiffItem, Migration, RenameHints, Schema,
        SchemaDiff, TableId, TablesDiffItem,
    },
};

#[derive(Parser, Debug)]
pub struct GenerateCommand {
    /// Name for the migration
    #[arg(short, long)]
    name: Option<String>,
}

/// Collects rename hints by interactively asking the user about potential renames
fn collect_rename_hints(previous_schema: &Schema, schema: &Schema) -> Result<RenameHints> {
    let mut hints = RenameHints::default();
    let mut ignored_tables = HashSet::<TableId>::new();
    let mut ignored_columns = HashMap::<TableId, HashSet<ColumnId>>::new();
    let mut ignored_indices = HashMap::<TableId, HashSet<IndexId>>::new();

    'main: loop {
        let diff = SchemaDiff::from(previous_schema, schema, &hints);

        // Check for table renames
        let dropped_tables: Vec<_> = diff
            .tables()
            .iter()
            .filter_map(|item| match item {
                TablesDiffItem::DropTable(table) if !ignored_tables.contains(&table.id) => {
                    Some(*table)
                }
                _ => None,
            })
            .collect();

        let added_tables: Vec<_> = diff
            .tables()
            .iter()
            .filter_map(|item| match item {
                TablesDiffItem::CreateTable(table) => Some(*table),
                _ => None,
            })
            .collect();

        // If there are both dropped and added tables, ask about potential renames
        if !dropped_tables.is_empty() && !added_tables.is_empty() {
            for dropped_table in &dropped_tables {
                let mut options = vec![format!("  Drop \"{}\" ✖", dropped_table.name)];
                for added_table in &added_tables {
                    options.push(format!(
                        "  Rename \"{}\" → \"{}\"",
                        dropped_table.name, added_table.name
                    ));
                }

                let selection = Select::with_theme(&dialoguer_theme())
                    .with_prompt(format!("  Table \"{}\" is missing", dropped_table.name))
                    .items(&options)
                    .default(0)
                    .interact()?;

                if selection == 0 {
                    // User confirmed it was dropped
                    ignored_tables.insert(dropped_table.id);
                } else {
                    // User indicated a rename (selection - 1 maps to added_tables index)
                    let to_table = added_tables[selection - 1];
                    drop(diff);
                    hints.add_table_hint(dropped_table.id, to_table.id);
                    continue 'main; // Regenerate diff with new hint
                }
            }
        }

        // Check for column and index renames within altered tables
        for item in diff.tables().iter() {
            if let TablesDiffItem::AlterTable {
                previous,
                next: _,
                columns,
                indices,
            } = item
            {
                // Handle column renames
                let dropped_columns: Vec<_> = columns
                    .iter()
                    .filter_map(|item| match item {
                        ColumnsDiffItem::DropColumn(column)
                            if !ignored_columns
                                .get(&previous.id)
                                .is_some_and(|set| set.contains(&column.id)) =>
                        {
                            Some(*column)
                        }
                        _ => None,
                    })
                    .collect();

                let added_columns: Vec<_> = columns
                    .iter()
                    .filter_map(|item| match item {
                        ColumnsDiffItem::AddColumn(column) => Some(*column),
                        _ => None,
                    })
                    .collect();

                if !dropped_columns.is_empty() && !added_columns.is_empty() {
                    for dropped_column in &dropped_columns {
                        let mut options = vec![format!("  Drop \"{}\" ✖", dropped_column.name)];
                        for added_column in &added_columns {
                            options.push(format!(
                                "  Rename \"{}\" → \"{}\"",
                                dropped_column.name, added_column.name
                            ));
                        }

                        let selection = Select::with_theme(&dialoguer_theme())
                            .with_prompt(format!(
                                "  Column \"{}\".\"{}\" is missing",
                                previous.name, dropped_column.name
                            ))
                            .items(&options)
                            .default(0)
                            .interact()?;

                        if selection == 0 {
                            // User confirmed it was dropped
                            ignored_columns
                                .entry(previous.id)
                                .or_default()
                                .insert(dropped_column.id);
                        } else {
                            // User indicated a rename
                            let next_column = added_columns[selection - 1];
                            drop(diff);
                            hints.add_column_hint(dropped_column.id, next_column.id);
                            continue 'main; // Regenerate diff with new hint
                        }
                    }
                }

                // Handle index renames
                let dropped_indices: Vec<_> = indices
                    .iter()
                    .filter_map(|item| match item {
                        IndicesDiffItem::DropIndex(index)
                            if !ignored_indices
                                .get(&previous.id)
                                .is_some_and(|set| set.contains(&index.id)) =>
                        {
                            Some(*index)
                        }
                        _ => None,
                    })
                    .collect();

                let added_indices: Vec<_> = indices
                    .iter()
                    .filter_map(|item| match item {
                        IndicesDiffItem::CreateIndex(index) => Some(*index),
                        _ => None,
                    })
                    .collect();

                if !dropped_indices.is_empty() && !added_indices.is_empty() {
                    for dropped_index in &dropped_indices {
                        let mut options = vec![format!("  Drop \"{}\" ✖", dropped_index.name)];
                        for added_index in &added_indices {
                            options.push(format!(
                                "  Rename \"{}\" → \"{}\"",
                                dropped_index.name, added_index.name
                            ));
                        }

                        let selection = Select::with_theme(&dialoguer_theme())
                            .with_prompt(format!(
                                "  Index \"{}\".\"{}\" is missing",
                                previous.name, dropped_index.name
                            ))
                            .items(&options)
                            .default(0)
                            .interact()?;

                        if selection == 0 {
                            // User confirmed it was dropped
                            ignored_indices
                                .entry(previous.id)
                                .or_default()
                                .insert(dropped_index.id);
                        } else {
                            // User indicated a rename
                            let to_index = added_indices[selection - 1];
                            drop(diff);
                            hints.add_index_hint(dropped_index.id, to_index.id);
                            continue 'main; // Regenerate diff with new hint
                        }
                    }
                }
            }
        }

        // No more potential renames to ask about
        break;
    }

    Ok(hints)
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

        let rename_hints = collect_rename_hints(&previous_schema, &schema)?;
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
        let migration_path = config.migration.get_migrations_dir().join(&migration_name);

        let migration = db.driver().generate_migration(&diff);

        history.add_migration(HistoryFileMigration {
            // Some databases only supported signed 64-bit integers.
            id: rand::thread_rng().gen_range(0..i64::MAX) as u64,
            name: migration_name.clone(),
            snapshot_name: snapshot_name.clone(),
            checksum: None,
        });

        let Migration::Sql(sql) = migration;
        std::fs::write(migration_path, sql)?;
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Created migration file: {}", migration_name)).dim()
        );

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
