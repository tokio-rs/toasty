use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use toasty_core::schema::db::Schema;
use toml_edit::{DocumentMut, InlineTable, Item, Table, value};

const LOCK_FILE_VERSION: u32 = 1;

/// Lock file containing the current database schema state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    /// Lock file format version
    version: u32,

    /// The database schema
    pub schema: Schema,
}

impl LockFile {
    /// Create a new lock file with the given schema
    pub fn new(schema: Schema) -> Self {
        Self {
            version: LOCK_FILE_VERSION,
            schema,
        }
    }

    /// Load a lock file from a TOML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path.as_ref())?;
        contents.parse()
    }

    /// Save the lock file to a TOML file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::write(path.as_ref(), self.to_string())?;
        Ok(())
    }
}

impl FromStr for LockFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let lock_file: LockFile = toml::from_str(s)?;

        // Validate version
        if lock_file.version != LOCK_FILE_VERSION {
            bail!(
                "Unsupported lock file version: {}. Expected version {}",
                lock_file.version,
                LOCK_FILE_VERSION
            );
        }

        Ok(lock_file)
    }
}

impl fmt::Display for LockFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let doc = self.to_toml_document().map_err(|_| fmt::Error)?;
        write!(f, "{}", doc)
    }
}

impl LockFile {
    fn to_toml_document(&self) -> Result<DocumentMut> {
        let mut doc = DocumentMut::new();

        // Add version
        doc["version"] = value(self.version as i64);

        // Add schema with tables as array of tables
        doc["schema"] = Item::Table(Table::new());

        // Create array of tables for [[schema.tables]]
        for table in &self.schema.tables {
            let mut table_entry = Table::new();
            table_entry["id"] = value(table.id.0 as i64);
            table_entry["name"] = value(&table.name);

            // Add columns as array of tables [[schema.tables.columns]]
            for column in &table.columns {
                let mut col_entry = Table::new();

                // Serialize column ID as inline table
                let mut col_id = InlineTable::new();
                col_id.insert("table", (column.id.table.0 as i64).into());
                col_id.insert("index", (column.id.index as i64).into());
                col_entry["id"] = value(col_id);

                col_entry["name"] = value(&column.name);
                col_entry["nullable"] = value(column.nullable);
                col_entry["primary_key"] = value(column.primary_key);
                col_entry["auto_increment"] = value(column.auto_increment);

                table_entry["columns"]
                    .or_insert(Item::ArrayOfTables(Default::default()))
                    .as_array_of_tables_mut()
                    .unwrap()
                    .push(col_entry);
            }

            // Add primary_key
            let mut pk_table = Table::new();

            // Serialize columns as array of inline tables
            let mut pk_columns = toml_edit::Array::new();
            for col_id in &table.primary_key.columns {
                let mut col_id_inline = InlineTable::new();
                col_id_inline.insert("table", (col_id.table.0 as i64).into());
                col_id_inline.insert("index", (col_id.index as i64).into());
                pk_columns.push(col_id_inline);
            }
            pk_table["columns"] = value(pk_columns);

            // Serialize index as inline table
            let mut index_inline = InlineTable::new();
            index_inline.insert("table", (table.primary_key.index.table.0 as i64).into());
            index_inline.insert("index", (table.primary_key.index.index as i64).into());
            pk_table["index"] = value(index_inline);

            table_entry["primary_key"] = Item::Table(pk_table);

            doc["schema"]["tables"]
                .or_insert(Item::ArrayOfTables(Default::default()))
                .as_array_of_tables_mut()
                .unwrap()
                .push(table_entry);
        }

        Ok(doc)
    }
}
