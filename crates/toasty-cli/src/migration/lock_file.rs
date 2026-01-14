use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use toasty_core::schema::db::Schema;
use toml_edit::{value, DocumentMut, InlineTable, Item, Table};

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

                // Serialize types using serde to toml::Value then convert
                let ty_value = toml::Value::try_from(&column.ty).map_err(|e| anyhow::anyhow!("{}", e))?;
                let ty_str = toml::to_string(&ty_value).map_err(|e| anyhow::anyhow!("{}", e))?;
                let ty_item: Item = ty_str.parse().map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
                col_entry["ty"] = ty_item;

                let storage_ty_value = toml::Value::try_from(&column.storage_ty).map_err(|e| anyhow::anyhow!("{}", e))?;
                let storage_ty_str = toml::to_string(&storage_ty_value).map_err(|e| anyhow::anyhow!("{}", e))?;
                let storage_ty_item: Item = storage_ty_str.parse().map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
                col_entry["storage_ty"] = storage_ty_item;

                table_entry["columns"]
                    .or_insert(Item::ArrayOfTables(Default::default()))
                    .as_array_of_tables_mut()
                    .unwrap()
                    .push(col_entry);
            }

            // Add indices as array of tables [[schema.tables.indices]]
            for index in &table.indices {
                let mut idx_entry = Table::new();

                // Serialize index ID as inline table
                let mut idx_id = InlineTable::new();
                idx_id.insert("table", (index.id.table.0 as i64).into());
                idx_id.insert("index", (index.id.index as i64).into());
                idx_entry["id"] = value(idx_id);

                idx_entry["name"] = value(&index.name);

                // Serialize 'on' (TableId) as inline table
                let mut on_id = InlineTable::new();
                on_id.insert("0", (index.on.0 as i64).into());
                idx_entry["on"] = value(on_id);

                // Serialize columns as inline array
                let columns_value = toml::Value::try_from(&index.columns).map_err(|e| anyhow::anyhow!("{}", e))?;
                let columns_str = toml::to_string(&columns_value).map_err(|e| anyhow::anyhow!("{}", e))?;
                let columns_item: Item = columns_str.parse().map_err(|e: toml_edit::TomlError| anyhow::anyhow!("parse error: {}", e))?;
                idx_entry["columns"] = columns_item;

                idx_entry["unique"] = value(index.unique);
                idx_entry["primary_key"] = value(index.primary_key);

                table_entry["indices"]
                    .or_insert(Item::ArrayOfTables(Default::default()))
                    .as_array_of_tables_mut()
                    .unwrap()
                    .push(idx_entry);
            }

            // Add primary_key
            let pk_value = toml::Value::try_from(&table.primary_key).map_err(|e| anyhow::anyhow!("{}", e))?;
            let pk_str = toml::to_string(&pk_value).map_err(|e| anyhow::anyhow!("{}", e))?;
            let pk_item: Item = pk_str.parse().map_err(|e: toml_edit::TomlError| anyhow::anyhow!("parse error: {}", e))?;
            table_entry["primary_key"] = pk_item;

            doc["schema"]["tables"]
                .or_insert(Item::ArrayOfTables(Default::default()))
                .as_array_of_tables_mut()
                .unwrap()
                .push(table_entry);
        }

        Ok(doc)
    }
}
