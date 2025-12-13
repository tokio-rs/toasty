use std::collections::HashMap;
use std::sync::Arc;
use tempfile::NamedTempFile;
use toasty::db;
use toasty::driver::Capability;
use toasty_driver_sqlite::Sqlite;

use crate::Setup;

pub struct SetupSqlite {
    _temp_file: NamedTempFile, // Keep alive for automatic cleanup
    temp_db_path: String,      // Path for connections
    driver: Arc<Sqlite>,       // Driver instance for TestDriver methods
}

impl SetupSqlite {
    pub fn new() -> Self {
        let temp_file =
            NamedTempFile::new().expect("Failed to create temporary file for SQLite test");

        // Get the path as a string for SQLite URL
        let temp_db_path = temp_file.path().display().to_string();

        // Create driver instance for TestDriver methods
        let driver = Sqlite::open(&temp_db_path)
            .expect("Failed to create SQLite driver");

        Self {
            _temp_file: temp_file,
            temp_db_path,
            driver: Arc::new(driver),
        }
    }

    /// Access the temporary database file path for raw database operations.
    /// This enables raw storage verification when the unsigned integer support is merged.
    pub fn temp_db_path(&self) -> &str {
        &self.temp_db_path
    }
}

impl Default for SetupSqlite {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Driver>> {
        let url = format!("sqlite:{}", self.temp_db_path);
        let conn = toasty::driver::Connection::connect(&url).await?;
        Ok(Box::new(conn))
    }

    fn configure_builder(&self, _builder: &mut db::Builder) {
        // SQLite doesn't need table prefixes for isolation
    }

    fn capability(&self) -> &Capability {
        &Capability::TEST_CAPABILITY
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
        use toasty_core::driver::TestDriver;
        self.driver.get_raw_column_value(table, column, filter).await
    }
}
