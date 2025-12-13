use std::collections::HashMap;
use std::sync::Arc;
use toasty::driver::Capability;
use toasty_core::stmt;
use toasty_driver_postgresql::PostgreSQL;
use tokio::sync::OnceCell;

use crate::{isolation::TestIsolation, Setup};

pub struct SetupPostgreSQL {
    isolation: TestIsolation,
    // Per-test-instance client to avoid runtime issues with static sharing
    client: OnceCell<Arc<tokio_postgres::Client>>,
    // Driver instance for TestDriver methods
    driver: OnceCell<Arc<PostgreSQL>>,
}

impl SetupPostgreSQL {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            client: OnceCell::new(),
            driver: OnceCell::new(),
        }
    }

    /// Get or create the per-test-instance PostgreSQL client
    async fn get_client(&self) -> &Arc<tokio_postgres::Client> {
        self.client
            .get_or_init(|| async {
                use tokio_postgres::NoTls;

                let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
                    .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

                let (client, connection) = tokio_postgres::connect(&url, NoTls)
                    .await
                    .unwrap_or_else(|e| panic!("PostgreSQL connection failed: {e}"));

                // Spawn the connection task
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("PostgreSQL connection error: {e}");
                    }
                });

                Arc::new(client)
            })
            .await
    }

    /// Get or create the per-test-instance PostgreSQL driver
    async fn get_driver(&self) -> &Arc<PostgreSQL> {
        self.driver
            .get_or_init(|| async {
                let client = self.get_client().await;
                // Clone the Arc to get a Client (not Arc<Client>)
                let client_inner = (**client).clone();
                Arc::new(PostgreSQL::new(client_inner))
            })
            .await
    }
}

impl Default for SetupPostgreSQL {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupPostgreSQL {
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Driver>> {
        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
        let conn = toasty::driver::Connection::connect(&url).await?;
        Ok(Box::new(conn))
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &Capability::POSTGRESQL
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_postgresql_tables_impl()
            .await
            .map_err(|e| toasty::Error::msg(format!("PostgreSQL cleanup failed: {e}")))
    }

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, stmt::Value>,
    ) -> toasty::Result<stmt::Value> {
        use toasty_core::driver::TestDriver;

        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);
        let driver = self.get_driver().await;
        driver.get_raw_column_value(&full_table_name, column, filter).await
    }
}

impl SetupPostgreSQL {
    /// Cleanup PostgreSQL tables using the cached connection
    async fn cleanup_postgresql_tables_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Reuse the cached client
        let client = self.get_client().await;

        let my_prefix = self.isolation.table_prefix();
        let escaped_prefix = my_prefix
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%");

        // Query for tables that belong to this test
        let rows = client
            .query(
                "SELECT table_name FROM information_schema.tables
             WHERE table_schema = 'public' AND table_name LIKE $1",
                &[&format!("{escaped_prefix}%")],
            )
            .await?;

        // Drop each table
        for row in rows {
            let table_name: String = row.get(0);
            let query = format!("DROP TABLE IF EXISTS {table_name}");
            let _ = client.simple_query(&query).await; // Ignore individual table drop errors
        }

        Ok(())
    }
}
