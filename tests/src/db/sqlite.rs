use tempfile::NamedTempFile;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupSqlite {
    _temp_file: NamedTempFile, // Keep alive for automatic cleanup
    temp_db_path: String,      // Path for connections
}

impl SetupSqlite {
    pub fn new() -> Self {
        let temp_file =
            NamedTempFile::new().expect("Failed to create temporary file for SQLite test");

        // Get the path as a string for SQLite URL
        let temp_db_path = temp_file.path().display().to_string();

        Self {
            _temp_file: temp_file,
            temp_db_path,
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
