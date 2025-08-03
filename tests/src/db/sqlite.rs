use std::sync::Mutex;
use tempfile::NamedTempFile;
use toasty::driver::Capability;
use toasty::{db, Db};

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
        let conn = self
            .raw_connection
            .lock()
            .map_err(|e| toasty::Error::msg(format!("Failed to acquire connection lock: {e}")))?;

        // Query the raw value from the database
        let query = format!("SELECT {column_name} FROM {table_name} WHERE id = ?");

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| toasty::Error::msg(format!("Failed to prepare query: {e}")))?;

        let raw_value: String = stmt
            .query_row([id_value], |row| row.get(0))
            .map_err(|e| toasty::Error::msg(format!("Failed to query raw value: {e}")))?;

        // Parse the raw value to the expected type
        raw_value.parse::<T>().map_err(|e| {
            toasty::Error::msg(format!("Failed to parse raw value '{raw_value}': {e:?}"))
        })
    }
}

impl Default for SetupSqlite {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        // Use temporary file database for consistent access across connections
        let url = format!("sqlite:{}", self.temp_db_path);
        builder.connect(&url).await
    }

    fn capability(&self) -> &Capability {
        &Capability::SQLITE
    }

    // Temporary file cleanup is handled automatically by NamedTempFile::drop()
    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        Ok(())
    }
}
