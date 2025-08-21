#[macro_use]
mod macros;

pub mod db;
mod db_test;
mod exec_log;
pub mod expr;
mod isolation;
mod logging_driver;

// Re-export for use in macros - needs to be public for macro expansion
pub use db_test::DbTest;
pub use exec_log::ExecLog;
pub use logging_driver::{DriverOp, LoggingDriver};

use std::collections::HashMap;
use toasty_core::driver::Capability;

pub use std_util::*;

/// Helper function to look up TableId by table name (handles database-specific prefixes)
pub fn table_id(db: &toasty::Db, table_name: &str) -> toasty_core::schema::db::TableId {
    let schema = db.schema();
    
    // First try exact match
    if let Some(position) = schema.db.tables.iter().position(|t| t.name == table_name) {
        return toasty_core::schema::db::TableId(position);
    }
    
    // If not found, try to find a table that ends with the given name (for database prefixes)
    if let Some(position) = schema.db.tables.iter().position(|t| t.name.ends_with(table_name)) {
        return toasty_core::schema::db::TableId(position);
    }
    
    // If still not found, show available tables for debugging
    let available_tables: Vec<_> = schema.db.tables.iter().map(|t| &t.name).collect();
    panic!("Table '{}' not found. Available tables: {:?}", table_name, available_tables);
}

/// Helper function to get a single ColumnId for specified table and column
pub fn column(
    db: &toasty::Db, 
    table_name: &str, 
    column_name: &str
) -> toasty_core::schema::db::ColumnId {
    columns(db, table_name, &[column_name])[0]
}

/// Helper function to generate a Vec<ColumnId> for specified table and columns
pub fn columns(
    db: &toasty::Db, 
    table_name: &str, 
    column_names: &[&str]
) -> Vec<toasty_core::schema::db::ColumnId> {
    let schema = db.schema();
    
    // Find the table using the same logic as table_id (handles prefixes)
    let table = schema.db.tables.iter()
        .find(|t| t.name == table_name || t.name.ends_with(table_name))
        .expect(&format!("Table '{}' not found", table_name));
    
    let table_id = table_id(db, table_name);
    
    column_names.iter().map(|col_name| {
        let index = table.columns.iter()
            .position(|c| c.name == *col_name)
            .expect(&format!("Column '{}' not found in table '{}'", col_name, table_name));
        
        toasty_core::schema::db::ColumnId { table: table_id, index }
    }).collect()
}

#[async_trait::async_trait]
pub trait Setup: Send + Sync + 'static {
    /// Create a connection to the database
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Driver>>;

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

#[macro_export]
macro_rules! models {
    (
        $( $model:ident ),*
    ) => {{
        let mut builder = toasty::Db::builder();
        $( builder.register::<$model>(); )*
        builder
    }};
}

#[macro_export]
macro_rules! tests {
    (
        $(
            $( #[$attrs:meta] )*
            $f:ident
        ),+
    ) => {
        #[cfg(feature = "dynamodb")]
        mod dynamodb {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::dynamodb::SetupDynamoDb::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "sqlite")]
        mod sqlite {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::sqlite::SetupSqlite::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "mysql")]
        mod mysql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::mysql::SetupMySQL::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }

        #[cfg(feature = "postgresql")]
        mod postgresql {
            $(
                #[test]
                $( #[$attrs] )*
                fn $f() {
                    let mut test = $crate::DbTest::new(
                        Box::new($crate::db::postgresql::SetupPostgreSQL::new())
                    );

                    test.run_test(move |test| Box::pin(async move {
                        super::$f(test).await;
                    }));
                }
            )*
        }
    };
    (
        $(
            $( #[$attrs:meta] )*
            $f:ident,
        )+
    ) => {
        $crate::tests!( $(
            $( #[$attrs] )*
            $f
        ),+ );
    }
}
