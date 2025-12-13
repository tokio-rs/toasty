use std::collections::HashMap;
use std::sync::Arc;
use toasty::driver::Capability;
use toasty_driver_dynamodb::DynamoDb;
use tokio::sync::OnceCell;

use crate::{isolation::TestIsolation, Setup};

pub struct SetupDynamoDb {
    isolation: TestIsolation,
    // Per-test-instance client to avoid runtime issues with static sharing
    client: OnceCell<aws_sdk_dynamodb::Client>,
    // Driver instance for TestDriver methods
    driver: OnceCell<Arc<DynamoDb>>,
}

impl SetupDynamoDb {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            client: OnceCell::new(),
            driver: OnceCell::new(),
        }
    }

    /// Get or create the per-test-instance DynamoDB client
    async fn get_client(&self) -> &aws_sdk_dynamodb::Client {
        self.client
            .get_or_init(|| async {
                use aws_config::BehaviorVersion;

                // Create DynamoDB client with test credentials (matching the driver setup)
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region("us-east-1")
                    .credentials_provider(aws_sdk_dynamodb::config::Credentials::for_tests())
                    .endpoint_url("http://localhost:8000")
                    .load()
                    .await;
                aws_sdk_dynamodb::Client::new(&config)
            })
            .await
    }

    /// Get or create the per-test-instance DynamoDB driver
    async fn get_driver(&self) -> &Arc<DynamoDb> {
        self.driver
            .get_or_init(|| async {
                let client = self.get_client().await.clone();
                Arc::new(DynamoDb::new(client))
            })
            .await
    }
}

impl Default for SetupDynamoDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    async fn connect(&self) -> toasty::Result<Box<dyn toasty_core::driver::Driver>> {
        let url =
            std::env::var("TOASTY_TEST_DYNAMODB_URL").unwrap_or_else(|_| "dynamodb://".to_string());
        let conn = toasty::driver::Connection::connect(&url).await?;
        Ok(Box::new(conn))
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &DynamoDB::CAPABILITY
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_dynamodb_tables_impl()
            .await
            .map_err(|e| toasty::Error::msg(format!("DynamoDB cleanup failed: {e}")))
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

impl SetupDynamoDb {
    /// Cleanup DynamoDB tables using the cached connection
    async fn cleanup_dynamodb_tables_impl(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Reuse the cached client
        let client = self.get_client().await;

        let my_prefix = self.isolation.table_prefix();

        // List all tables and filter for ones we own
        let mut table_names = Vec::new();
        let mut exclusive_start_table_name = None;

        loop {
            let mut request = client.list_tables().limit(100);

            if let Some(start_name) = exclusive_start_table_name {
                request = request.exclusive_start_table_name(start_name);
            }

            let response = request.send().await?;

            if let Some(names) = response.table_names {
                for name in names {
                    if name.starts_with(&my_prefix) {
                        table_names.push(name);
                    }
                }
            }

            exclusive_start_table_name = response.last_evaluated_table_name;
            if exclusive_start_table_name.is_none() {
                break;
            }
        }

        // Delete each table
        for table_name in table_names {
            let _ = client.delete_table().table_name(table_name).send().await; // Ignore individual table deletion errors
        }

        Ok(())
    }
}
