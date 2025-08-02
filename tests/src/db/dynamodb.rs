use std::collections::HashMap;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, RawValue, Setup};

pub struct SetupDynamoDb {
    isolation: TestIsolation,
}

impl SetupDynamoDb {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
        }
    }
}

impl Default for SetupDynamoDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    /// Try building the full schema and connecting to the database.
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        let prefix = self.isolation.table_prefix();

        let url =
            std::env::var("TOASTY_TEST_DYNAMODB_URL").unwrap_or_else(|_| "dynamodb://".to_string());

        builder.table_name_prefix(&prefix).connect(&url).await
    }

    fn capability(&self) -> &Capability {
        &Capability::DYNAMODB
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        cleanup_dynamodb_tables(&self.isolation)
            .await
            .map_err(|e| toasty::Error::msg(format!("DynamoDB cleanup failed: {e}")))
    }

    async fn get_raw_column_value<T>(
        &self,
        _table: &str,
        _column: &str,
        _filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: RawValue,
    {
        Err(toasty::Error::msg(
            "DynamoDB raw value access not yet implemented",
        ))
    }
}

async fn cleanup_dynamodb_tables(
    isolation: &TestIsolation,
) -> Result<(), Box<dyn std::error::Error>> {
    use aws_config::BehaviorVersion;
    use aws_sdk_dynamodb::Client;

    // Create DynamoDB client
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = Client::new(&config);

    let my_prefix = isolation.table_prefix();

    // List all tables and filter for ones we own
    let mut table_names = Vec::new();
    let mut exclusive_start_table_name = None;

    loop {
        let mut request = client.list_tables();
        if let Some(start_name) = exclusive_start_table_name {
            request = request.exclusive_start_table_name(start_name);
        }

        let response = request.send().await?;

        if let Some(names) = response.table_names {
            // Filter for tables that belong to this test
            table_names.extend(
                names
                    .into_iter()
                    .filter(|name| name.starts_with(&my_prefix)),
            );
        }

        exclusive_start_table_name = response.last_evaluated_table_name;
        if exclusive_start_table_name.is_none() {
            break;
        }
    }

    // Delete our tables
    for table_name in table_names {
        // First check if table exists and is not being deleted
        match client.describe_table().table_name(&table_name).send().await {
            Ok(response) => {
                if let Some(table) = response.table {
                    if let Some(status) = table.table_status {
                        // Only try to delete if table is ACTIVE
                        if status == aws_sdk_dynamodb::types::TableStatus::Active {
                            let _ = client.delete_table().table_name(&table_name).send().await;
                            // Ignore individual delete errors
                        }
                    }
                }
            }
            Err(_) => {
                // Table doesn't exist or we can't access it, skip
                continue;
            }
        }
    }

    Ok(())
}
