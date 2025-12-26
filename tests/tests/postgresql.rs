#![cfg(feature = "postgresql")]

use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio_postgres::NoTls;

struct PostgreSqlSetup {
    client: OnceCell<Arc<tokio_postgres::Client>>,
}

impl PostgreSqlSetup {
    fn new() -> Self {
        Self {
            client: OnceCell::new(),
        }
    }

    async fn get_client(&self) -> &Arc<tokio_postgres::Client> {
        self.client
            .get_or_init(|| async {
                let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
                    .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

                let (client, connection) = tokio_postgres::connect(&url, NoTls)
                    .await
                    .expect("Failed to connect to PostgreSQL");

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("connection error: {}", e);
                    }
                });

                Arc::new(client)
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for PostgreSqlSetup {
    fn driver(&self) -> Box<dyn toasty::driver::Driver> {
        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());
        Box::new(toasty::db::Connect::new(&url).expect("Failed to create PostgreSQL driver"))
    }

    async fn delete_table(&self, name: &str) {
        let client = self.get_client().await;

        let sql = format!("DROP TABLE IF EXISTS \"{}\" CASCADE", name);
        client
            .execute(&sql, &[])
            .await
            .expect("Failed to drop table");
    }
}

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(PostgreSqlSetup::new(), bigdecimal_implemented: false);
