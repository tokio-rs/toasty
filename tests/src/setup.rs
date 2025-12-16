use std::collections::HashMap;
use toasty::driver::Capability;

pub trait Setup: Send + Sync + 'static {
    /// Create a connection to the database
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Connection>>;

    /// Configure the builder with database-specific settings (like table prefixes)
    fn configure_builder(&self, _builder: &mut toasty::db::Builder) {
        // Default: no configuration needed (SQLite)
        // Other databases override this to add table prefixes
    }

    fn capability(&self) -> &Capability;

    /// Clean up tables created by this specific setup instance.
    ///
    /// This method should drop only the tables that belong to this test,
    /// identified by the table prefix used during setup.
    async fn cleanup_my_tables(&self) -> toasty::Result<()>;

    /// Get the raw value stored in the database for verification
    ///
    /// - `table`: Table name WITHOUT prefix (e.g., "foo", not "test_123_foo")
    /// - `column`: Column name to retrieve (e.g., "val")
    /// - `filter`: WHERE clause conditions as column_name -> value pairs
    /// Returns the raw `Value` from the database
    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<toasty_core::stmt::Value>;
}
