use crate::{Config, theme::dialoguer_theme};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Select;
use hashbrown::{HashMap, HashSet};
use toasty::{
    Db,
    schema::db::{
        ColumnId, ColumnsDiffItem, IndexId, IndicesDiffItem, RenameHints, Schema, SchemaDiff,
        TableId, TablesDiffItem,
    },
};

/// Generates a new SQL migration from the current schema diff.
///
/// Compares the current database schema (as registered on the [`Db`]) against
/// the most recent snapshot. If there are differences, generates a SQL
/// migration file, writes a new snapshot, and updates the history file.
///
/// When the diff contains dropped-and-added tables, columns, or indices, the
/// command interactively asks whether these are renames rather than
/// drop-then-create pairs.
///
/// If no schema changes are detected, the command exits without creating any
/// files.
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

        let previous_schema = toasty::migrate::previous_schema(&config.migration)?;
        let current_schema = Schema::clone(&db.schema().db);

        let rename_hints = collect_rename_hints(&previous_schema, &current_schema)?;

        let generated =
            toasty::migrate::generate(db, &config.migration, self.name.as_deref(), &rename_hints)?;

        let Some(generated) = generated else {
            println!(
                "  {}",
                style("The current schema matches the previous snapshot. No migration needed.")
                    .magenta()
                    .dim()
            );
            println!();
            return Ok(());
        };

        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!(
                "Created migration file: {}",
                generated.migration_name
            ))
            .dim()
        );
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Created snapshot: {}", generated.snapshot_name)).dim()
        );
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
                generated.migration_name
            ))
            .green()
            .bold()
        );
        println!();

        Ok(())
    }
}
