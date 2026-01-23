use std::collections::HashMap;
use toasty::{
    db::Connect,
    driver::{Capability, Driver},
};
use tokio::sync::OnceCell;

use crate::{isolation::TestIsolation, Setup};

pub struct SetupDynamoDb {
    isolation: TestIsolation,
    // Per-test-instance client to avoid runtime issues with static sharing
    client: OnceCell<aws_sdk_dynamodb::Client>,
}

impl SetupDynamoDb {
    pub fn new() -> Self {
        Self {
            isolation: TestIsolation::new(),
            client: OnceCell::new(),
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
}

impl Default for SetupDynamoDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    fn driver(&self) -> Box<dyn Driver> {
        let url =
            std::env::var("TOASTY_TEST_DYNAMODB_URL").unwrap_or_else(|_| "dynamodb://".to_string());
        Box::new(Connect::new(&url).unwrap())
    }

    fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let prefix = self.isolation.table_prefix();
        builder.table_name_prefix(&prefix);
    }

    fn capability(&self) -> &Capability {
        &Capability::DYNAMODB
    }

    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        self.cleanup_dynamodb_tables_impl()
            .await
            .map_err(|e| toasty::err!("DynamoDB cleanup failed: {e}"))
    }

    async fn get_raw_column_value(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);

        // Get the per-test-instance DynamoDB client
        let client = self.get_client().await;

        // Convert filter to DynamoDB key
        let mut key = HashMap::new();
        for (col_name, value) in filter {
            let attr_value = self.stmt_value_to_dynamodb_attr(&value)?;
            key.insert(col_name, attr_value);
        }

        // Get item from DynamoDB
        let response = client
            .get_item()
            .table_name(&full_table_name)
            .set_key(Some(key))
            .send()
            .await
            .unwrap_or_else(|e| panic!("DynamoDB get_item failed: {e}"));

        if let Some(item) = response.item {
            if let Some(attr_value) = item.get(column) {
                self.dynamodb_attr_to_stmt_value(attr_value)
            } else {
                panic!("Column '{column}' not found in DynamoDB item")
            }
        } else {
            panic!("No item found in DynamoDB")
        }
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

    fn stmt_value_to_dynamodb_attr(
        &self,
        value: &toasty_core::stmt::Value,
    ) -> toasty::Result<aws_sdk_dynamodb::types::AttributeValue> {
        use aws_sdk_dynamodb::types::AttributeValue;

        match value {
            toasty_core::stmt::Value::String(s) => Ok(AttributeValue::S(s.clone())),
            toasty_core::stmt::Value::I64(i) => Ok(AttributeValue::N(i.to_string())),
            toasty_core::stmt::Value::U64(u) => Ok(AttributeValue::N(u.to_string())),
            toasty_core::stmt::Value::I32(i) => Ok(AttributeValue::N(i.to_string())),
            toasty_core::stmt::Value::I16(i) => Ok(AttributeValue::N(i.to_string())),
            toasty_core::stmt::Value::I8(i) => Ok(AttributeValue::N(i.to_string())),
            toasty_core::stmt::Value::U32(u) => Ok(AttributeValue::N(u.to_string())),
            toasty_core::stmt::Value::U16(u) => Ok(AttributeValue::N(u.to_string())),
            toasty_core::stmt::Value::U8(u) => Ok(AttributeValue::N(u.to_string())),
            toasty_core::stmt::Value::Bool(b) => Ok(AttributeValue::Bool(*b)),
            toasty_core::stmt::Value::Id(id) => Ok(AttributeValue::S(id.to_string())),
            toasty_core::stmt::Value::Null => Ok(AttributeValue::Null(true)),
            _ => todo!("Unsupported stmt::Value type for DynamoDB: {value:?}"),
        }
    }

    fn dynamodb_attr_to_stmt_value(
        &self,
        attr: &aws_sdk_dynamodb::types::AttributeValue,
    ) -> toasty::Result<toasty_core::stmt::Value> {
        use aws_sdk_dynamodb::types::AttributeValue;

        match attr {
            AttributeValue::S(s) => Ok(toasty_core::stmt::Value::String(s.clone())),
            AttributeValue::N(n) => {
                // Try to parse as i64 first, fall back to u64 if it's too large/unsigned
                if let Ok(val) = n.parse::<i64>() {
                    Ok(toasty_core::stmt::Value::I64(val))
                } else if let Ok(val) = n.parse::<u64>() {
                    Ok(toasty_core::stmt::Value::U64(val))
                } else {
                    Err(toasty::err!("Failed to parse DynamoDB number: {}", n))
                }
            }
            AttributeValue::B(b) => Ok(toasty_core::stmt::Value::Bytes(b.clone().into_inner())),
            AttributeValue::Bool(b) => Ok(toasty_core::stmt::Value::Bool(*b)),
            AttributeValue::Null(_) => Ok(toasty_core::stmt::Value::Null),
            _ => todo!("Unsupported DynamoDB AttributeValue type: {attr:?}"),
        }
    }
}
