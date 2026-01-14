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

            // Add indices as array of tables [[schema.tables.indices]]
            for index in &table.indices {
                let mut index_entry = Table::new();

                // Serialize index ID as inline table
                let mut index_id = InlineTable::new();
                index_id.insert("table", (index.id.table.0 as i64).into());
                index_id.insert("index", (index.id.index as i64).into());
                index_entry["id"] = value(index_id);

                index_entry["name"] = value(&index.name);
                index_entry["on"] = value(index.on.0 as i64);
                index_entry["unique"] = value(index.unique);
                index_entry["primary_key"] = value(index.primary_key);

                // Add columns as array of inline tables
                let mut index_columns = toml_edit::Array::new();
                for index_col in &index.columns {
                    let mut index_col_inline = InlineTable::new();

                    // Serialize column ID as inline table
                    let mut col_id = InlineTable::new();
                    col_id.insert("table", (index_col.column.table.0 as i64).into());
                    col_id.insert("index", (index_col.column.index as i64).into());
                    index_col_inline.insert("column", col_id.into());

                    // Serialize op
                    match index_col.op {
                        toasty_core::schema::db::IndexOp::Eq => {
                            index_col_inline.insert("op", "Eq".into());
                        }
                        toasty_core::schema::db::IndexOp::Sort(dir) => {
                            let mut sort_inline = InlineTable::new();
                            let dir_str = match dir {
                                toasty_core::stmt::Direction::Asc => "Asc",
                                toasty_core::stmt::Direction::Desc => "Desc",
                            };
                            sort_inline.insert("Sort", dir_str.into());
                            index_col_inline.insert("op", sort_inline.into());
                        }
                    }

                    // Serialize scope
                    let scope_str = match index_col.scope {
                        toasty_core::schema::db::IndexScope::Partition => "Partition",
                        toasty_core::schema::db::IndexScope::Local => "Local",
                    };
                    index_col_inline.insert("scope", scope_str.into());

                    index_columns.push(index_col_inline);
                }
                index_entry["columns"] = value(index_columns);

                table_entry["indices"]
                    .or_insert(Item::ArrayOfTables(Default::default()))
                    .as_array_of_tables_mut()
                    .unwrap()
                    .push(index_entry);
            }

            doc["schema"]["tables"]
                .or_insert(Item::ArrayOfTables(Default::default()))
                .as_array_of_tables_mut()
                .unwrap()
                .push(table_entry);
        }

        Ok(doc)
    }
}
