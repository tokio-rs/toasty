use std::collections::HashMap;
use std::sync::Mutex;
use tempfile::NamedTempFile;
use toasty::{
    db::{self, Connect},
    driver::{Capability, Driver},
};

use crate::Setup;

pub struct SetupSqlite {
    _temp_file: NamedTempFile, // Keep alive for automatic cleanup
    temp_db_path: String,      // Path for connections
    raw_connection: Mutex<rusqlite::Connection>, // Shared connection for raw access
}

impl SetupSqlite {
    pub fn new() -> Self {
        let temp_file =
            NamedTempFile::new().expect("Failed to create temporary file for SQLite test");

        // Get the path as a string for SQLite URL
        let temp_db_path = temp_file.path().display().to_string();

        // Create a raw connection for get_raw_column_value operations
        let raw_connection = rusqlite::Connection::open(&temp_db_path)
            .expect("Failed to create raw SQLite connection");

        Self {
            _temp_file: temp_file,
            temp_db_path,
            raw_connection: Mutex::new(raw_connection),
        }
    }

    /// Access the temporary database file path for raw database operations.
    /// This enables raw storage verification when the unsigned integer support is merged.
    pub fn temp_db_path(&self) -> &str {
        &self.temp_db_path
    }

    /// Get raw column value from the database for verification purposes.
    /// This method uses a shared connection to efficiently retrieve
    /// the actual stored value, enabling raw storage verification.
    pub async fn get_raw_column_value<T>(
        &self,
        table_name: &str,
        column_name: &str,
        id_value: i64,
    ) -> toasty::Result<T>
    where
        T: std::str::FromStr + Send,
        T::Err: std::fmt::Debug,
    {
        // No spawn_blocking needed here because:
        // 1. This is just a test runner, not production code
        // 2. SQLite file operations are typically fast and often cached in memory
        // 3. Test database files are small and local
        let conn = self.raw_connection.lock().map_err(|e| {
            toasty::Error::from_args(format_args!("Failed to acquire connection lock: {e}"))
        })?;

        // Query the raw value from the database
        let query = format!("SELECT {column_name} FROM {table_name} WHERE id = ?");

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| toasty::Error::from_args(format_args!("Failed to prepare query: {e}")))?;

        let raw_value: String = stmt.query_row([id_value], |row| row.get(0)).map_err(|e| {
            toasty::Error::from_args(format_args!("Failed to query raw value: {e}"))
        })?;

        // Parse the raw value to the expected type
        raw_value.parse::<T>().map_err(|e| {
            toasty::Error::from_args(format_args!(
                "Failed to parse raw value '{raw_value}': {e:?}"
            ))
        })
    }

    /// Helper method to convert SQLite row values to stmt::Value for unsigned integer support
    fn sqlite_row_to_stmt_value(
        &self,
        row: &rusqlite::Row,
        col: usize,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use rusqlite::types::ValueRef;

        let value_ref = row.get_ref(col).map_err(|e| {
            toasty::Error::from_args(format_args!("SQLite column access failed: {e}"))
        })?;

        match value_ref {
            ValueRef::Integer(i) => Ok(toasty_core::stmt::Value::I64(i)),
            ValueRef::Text(s) => {
                let text = std::str::from_utf8(s)
                    .unwrap_or_else(|e| panic!("SQLite text conversion failed: {e}"));
                Ok(toasty_core::stmt::Value::String(text.to_string()))
            }
            ValueRef::Null => Ok(toasty_core::stmt::Value::Null),
            _ => todo!(
                "SQLite value type conversion not yet implemented: {:?}",
                value_ref
            ),
        }
    }
}

impl Default for SetupSqlite {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    fn driver(&self) -> Box<dyn Driver> {
        Box::new(Connect::new(&format!("sqlite:{}", self.temp_db_path)).unwrap())
    }

    fn configure_builder(&self, _builder: &mut db::Builder) {
        // SQLite doesn't need table prefixes for isolation
    }

    fn capability(&self) -> &Capability {
        &Capability::SQLITE
    }

    // Temporary file cleanup is handled automatically by NamedTempFile::drop()
    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        Ok(())
    }

    /// Get raw column value from the database for verification purposes.
    /// This method supports the unsigned integer testing by providing access to raw stored values.
    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        // Build WHERE clause from filter
        let mut where_conditions = Vec::new();
        let mut sqlite_params = Vec::new();

        for (col_name, value) in filter {
            where_conditions.push(format!("{col_name} = ?"));

            // Convert stmt::Value to SQLite parameter
            match value {
                toasty_core::stmt::Value::String(s) => sqlite_params.push(s),
                toasty_core::stmt::Value::I64(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::U64(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::I32(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::I16(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::I8(i) => sqlite_params.push(i.to_string()),
                toasty_core::stmt::Value::U32(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::U16(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::U8(u) => sqlite_params.push(u.to_string()),
                toasty_core::stmt::Value::Bool(b) => {
                    sqlite_params.push(if b { "1".to_string() } else { "0".to_string() })
                }
                toasty_core::stmt::Value::Id(id) => sqlite_params.push(id.to_string()),
                _ => todo!("Unsupported filter value type for SQLite: {value:?}"),
            }
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let query = format!("SELECT {column} FROM {table}{where_clause}");

        // Use the shared connection for efficient access
        let conn = self
            .raw_connection
            .lock()
            .unwrap_or_else(|e| panic!("Failed to acquire connection lock: {e}"));

        let mut stmt = conn
            .prepare(&query)
            .unwrap_or_else(|e| panic!("SQLite prepare failed: {e}"));

        let string_params: Vec<&str> = sqlite_params.iter().map(|s| s.as_str()).collect();
        let params_refs: Vec<&dyn rusqlite::ToSql> = string_params
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();

        let mut rows = stmt
            .query(&params_refs[..])
            .unwrap_or_else(|e| panic!("SQLite query failed: {e}"));

        if let Some(row) = rows
            .next()
            .unwrap_or_else(|e| panic!("SQLite row fetch failed: {e}"))
        {
            self.sqlite_row_to_stmt_value(row, 0)
        } else {
            panic!("No rows found")
        }
    }
}
