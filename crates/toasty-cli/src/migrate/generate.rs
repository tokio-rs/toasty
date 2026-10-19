use crate::{Flavor, Project, extract, theme::dialoguer_theme};
use anyhow::Result;
use clap::Parser;
use console::style;
use dialoguer::Select;
use hashbrown::{HashMap, HashSet};
use rand::RngExt;
use std::fs;
use toasty::migration::{self, History, HistoryEntry, Snapshot};
use toasty::schema::{
    db::{ColumnId, IndexId, Migration, Schema, TableId},
    diff,
};

/// Generates a new SQL migration from the current schema diff.
///
/// Compiles the target package, extracts its schema, and compares it against
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
    /// Database flavor to generate the migration for
    #[arg(long, value_enum)]
    flavor: Option<Flavor>,

    /// Name for the migration
    #[arg(short, long)]
    name: Option<String>,

    /// Bin target to extract the schema from, when the package has several
    #[arg(long)]
    bin: Option<String>,
}

/// A rename the user confirmed, to be added to the hints.
enum Rename {
    Table(TableId, TableId),
    Column(ColumnId, ColumnId),
    Index(IndexId, IndexId),
}

/// Drops the user has already confirmed, so they are not asked about again.
#[derive(Default)]
struct ConfirmedDrops {
    tables: HashSet<TableId>,
    columns: HashMap<TableId, HashSet<ColumnId>>,
    indices: HashMap<TableId, HashSet<IndexId>>,
}

/// Collects rename hints by interactively asking the user about potential renames
fn collect_rename_hints(previous_schema: &Schema, schema: &Schema) -> Result<diff::RenameHints> {
    let mut hints = diff::RenameHints::default();
    let mut confirmed = ConfirmedDrops::default();

    // Every confirmed rename changes the diff, so it is regenerated and the
    // remaining questions asked against the new one.
    loop {
        let diff = diff::Schema::from(previous_schema, schema, &hints);
        let Some(rename) = ask_next_rename(&diff, &mut confirmed)? else {
            break;
        };
        drop(diff);

        match rename {
            Rename::Table(from, to) => hints.add_table_hint(from, to),
            Rename::Column(from, to) => hints.add_column_hint(from, to),
            Rename::Index(from, to) => hints.add_index_hint(from, to),
        }
    }

    Ok(hints)
}

/// Walks the diff asking about each dropped table, column, and index that has
/// an added counterpart, until the user picks a rename. Returns `None` once
/// every drop has been confirmed.
fn ask_next_rename(
    diff: &diff::Schema<'_>,
    confirmed: &mut ConfirmedDrops,
) -> Result<Option<Rename>> {
    let added_tables: Vec<_> = diff
        .tables()
        .iter()
        .filter_map(|item| match item {
            diff::Table::Create(table) => Some(*table),
            _ => None,
        })
        .collect();

    if !added_tables.is_empty() {
        for item in diff.tables() {
            let diff::Table::Drop(dropped) = item else {
                continue;
            };
            if confirmed.tables.contains(&dropped.id) {
                continue;
            }

            let prompt = format!("  Table \"{}\" is missing", dropped.name);
            let names: Vec<_> = added_tables.iter().map(|t| t.name.as_str()).collect();
            match ask_rename(prompt, &dropped.name, &names)? {
                Some(to) => return Ok(Some(Rename::Table(dropped.id, added_tables[to].id))),
                None => {
                    confirmed.tables.insert(dropped.id);
                }
            }
        }
    }

    for item in diff.tables() {
        let diff::Table::Alter {
            previous,
            columns,
            indices,
            ..
        } = item
        else {
            continue;
        };

        let added_columns: Vec<_> = columns
            .iter()
            .filter_map(|item| match item {
                diff::Column::Add(column) => Some(*column),
                _ => None,
            })
            .collect();

        if !added_columns.is_empty() {
            let confirmed = confirmed.columns.entry(previous.id).or_default();
            for item in columns {
                let diff::Column::Drop(dropped) = item else {
                    continue;
                };
                if confirmed.contains(&dropped.id) {
                    continue;
                }

                let prompt = format!(
                    "  Column \"{}\".\"{}\" is missing",
                    previous.name, dropped.name
                );
                let names: Vec<_> = added_columns.iter().map(|c| c.name.as_str()).collect();
                match ask_rename(prompt, &dropped.name, &names)? {
                    Some(to) => {
                        return Ok(Some(Rename::Column(dropped.id, added_columns[to].id)));
                    }
                    None => {
                        confirmed.insert(dropped.id);
                    }
                }
            }
        }

        let added_indices: Vec<_> = indices
            .iter()
            .filter_map(|item| match item {
                diff::Index::Create(index) => Some(*index),
                _ => None,
            })
            .collect();

        if !added_indices.is_empty() {
            let confirmed = confirmed.indices.entry(previous.id).or_default();
            for item in indices {
                let diff::Index::Drop(dropped) = item else {
                    continue;
                };
                if confirmed.contains(&dropped.id) {
                    continue;
                }

                let prompt = format!(
                    "  Index \"{}\".\"{}\" is missing",
                    previous.name, dropped.name
                );
                let names: Vec<_> = added_indices.iter().map(|i| i.name.as_str()).collect();
                match ask_rename(prompt, &dropped.name, &names)? {
                    Some(to) => return Ok(Some(Rename::Index(dropped.id, added_indices[to].id))),
                    None => {
                        confirmed.insert(dropped.id);
                    }
                }
            }
        }
    }

    Ok(None)
}

/// Asks whether `dropped` was really dropped or renamed to one of `added`.
///
/// Returns the index into `added` of the chosen rename, or `None` when the
/// user confirmed the drop.
fn ask_rename(prompt: String, dropped: &str, added: &[&str]) -> Result<Option<usize>> {
    let mut options = vec![format!("  Drop \"{dropped}\" ✖")];
    options.extend(
        added
            .iter()
            .map(|name| format!("  Rename \"{dropped}\" → \"{name}\"")),
    );

    let selection = Select::with_theme(&dialoguer_theme())
        .with_prompt(prompt)
        .items(&options)
        .default(0)
        .interact()?;

    Ok(selection.checked_sub(1))
}

impl GenerateCommand {
    pub(crate) fn run(self, project: &Project) -> Result<()> {
        println!();
        println!(
            "  {}",
            style("Generate Migration").cyan().bold().underlined()
        );
        println!();

        let flavor = project.flavor(self.flavor)?;
        let schema = extract::extract_schema(project, flavor, self.bin.as_deref())?;

        crate::Config::create_if_missing(&project.package_root)?;

        let history_path = project.history_file_path();

        fs::create_dir_all(project.migrations_dir())?;
        fs::create_dir_all(project.snapshots_dir())?;
        fs::create_dir_all(history_path.parent().unwrap())?;

        let mut history = History::load_or_default(&history_path)?;

        let previous_snapshot = history
            .entries()
            .last()
            .map(|f| Snapshot::load(project.snapshots_dir().join(&f.snapshot_name)))
            .transpose()?;
        let previous_schema = previous_snapshot
            .map(|snapshot| snapshot.schema)
            .unwrap_or_else(Schema::default);

        let rename_hints = collect_rename_hints(&previous_schema, &schema)?;
        let Some(generated) = migration::generate(
            flavor.capability(),
            &previous_schema,
            &schema,
            &rename_hints,
        ) else {
            println!(
                "  {}",
                style("The current schema matches the previous snapshot. No migration needed.")
                    .magenta()
                    .dim()
            );
            println!();
            return Ok(());
        };

        let migration_prefix = match project.config.migration.prefix_style {
            crate::MigrationPrefixStyle::Sequential => {
                format!("{:04}", history.next_migration_number())
            }
            crate::MigrationPrefixStyle::Timestamp => {
                jiff::Timestamp::now().strftime("%Y%m%d_%H%M%S").to_string()
            }
        };
        let snapshot_name = format!("{migration_prefix}_snapshot.toml");
        let snapshot_path = project.snapshots_dir().join(&snapshot_name);

        let migration_name = format!(
            "{}_{}.sql",
            migration_prefix,
            self.name.as_deref().unwrap_or("migration")
        );
        let migration_path = project.migrations_dir().join(&migration_name);

        history.add_entry(HistoryEntry {
            // Some databases only supported signed 64-bit integers.
            id: rand::rng().random_range(0..i64::MAX) as u64,
            name: migration_name.clone(),
            snapshot_name: snapshot_name.clone(),
            checksum: None,
        });

        let snapshot = generated.snapshot.to_toml_string()?;
        let migration = generated.migration;
        let Migration::Sql(sql) = migration;
        std::fs::write(&migration_path, format!("{sql}\n"))?;
        println!(
            "  {} {}",
            style("✓").green().bold(),
            style(format!("Created migration file: {}", migration_name)).dim()
        );

        std::fs::write(&snapshot_path, snapshot)?;
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
