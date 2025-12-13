use std::collections::HashMap;
use std::sync::Arc;
use toasty::driver::Capability;
use toasty_driver_mysql::MySQL;

use crate::{isolation::TestIsolation, Setup};

pub struct SetupMySQL {
    isolation: TestIsolation,
    // Per-test-instance pool to avoid runtime issues with static sharing
    pool: tokio::sync::OnceCell<mysql_async::Pool>,
    // Driver instance for TestDriver methods
    driver: tokio::sync::OnceCell<Arc<MySQL>>,
}

impl SetupMySQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            pool: tokio::sync::OnceCell::new(),
            driver: tokio::sync::OnceCell::new(),
        }
    }

    /// Get or create the per-test-instance MySQL connection pool
    async fn get_pool(&self) -> &mysql_async::Pool {
        self.pool
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_MYSQL_URL")
                    .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());

                mysql_async::Pool::new(url.as_str())
            })
            .await
    }

    /// Get or create the per-test-instance MySQL driver
    async fn get_driver(&self) -> &Arc<MySQL> {
        self.driver
            .get_or_init(|| async {
                let pool = self.get_pool().await.clone();
                Arc::new(MySQL::new(pool))
            })
            .await
    }
}

impl Default for SetupMySQL {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupMySQL {
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Driver>> {
        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());
        let conn = toasty::driver::Connection::connect(&url).await?;
        Ok(Box::new(conn))
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &Capability::MYSQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_mysql_tables_impl()
            .await
            .map_err(|e| toasty::Error::msg(format!("MySQL cleanup failed: {e}")))
    }

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use toasty_core::driver::TestDriver;

        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);
        let driver = self.get_driver().await;
        driver.get_raw_column_value(&full_table_name, column, filter).await
    }
}

impl SetupMySQL {
    /// Cleanup MySQL tables using the cached connection
    async fn cleanup_mysql_tables_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        use mysql_async::prelude::Queryable;

        // Reuse the cached connection pool
        let pool = self.get_pool().await;
        let mut conn = pool.get_conn().await?;

        let my_prefix = self.isolation.table_prefix();
        let escaped_prefix = my_prefix
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%");

        // Query for tables that belong to this test
        let rows: Vec<String> = conn
            .query(format!(
                "SELECT table_name FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name LIKE '{escaped_prefix}%'"
            ))
            .await?;

        // Drop each table
        for table_name in rows {
            let query = format!("DROP TABLE IF EXISTS {table_name}");
            let _ = conn.query_drop(&query).await; // Ignore individual table drop errors
        }

        Ok(())
    }
}
