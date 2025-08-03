use std::collections::HashMap;
use toasty::driver::Capability;
use toasty::{db, Db};

use crate::{isolation::TestIsolation, Setup};

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
        table: &str,
        column: &str,
        filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: TryFrom<toasty_core::stmt::Value, Error = toasty_core::Error>,
    {
        use aws_config::BehaviorVersion;
        use aws_sdk_dynamodb::Client;

        let full_table_name = format!("{}{}", self.isolation.table_prefix(), table);

        // Create DynamoDB client with test credentials (matching the driver setup)
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region("foo")
            .credentials_provider(aws_sdk_dynamodb::config::Credentials::for_tests())
            .endpoint_url("http://localhost:8000")
            .load()
            .await;
        let client = Client::new(&config);

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
            .map_err(|e| toasty::Error::msg(format!("DynamoDB get_item failed: {e}")))?;

        if let Some(item) = response.item {
            if let Some(attr_value) = item.get(column) {
                let stmt_value = self.dynamodb_attr_to_stmt_value(attr_value)?;
                stmt_value.try_into().map_err(|e: toasty_core::Error| {
                    toasty::Error::msg(format!("Validation failed: {e}"))
                })
            } else {
                Err(toasty::Error::msg(format!(
                    "Column '{column}' not found in DynamoDB item"
                )))
            }
        } else {
            Err(toasty::Error::msg("No item found in DynamoDB"))
        }
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

impl SetupDynamoDb {
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
            _ => Err(toasty::Error::msg(format!(
                "Unsupported stmt::Value type for DynamoDB: {value:?}"
            ))),
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
                // DynamoDB stores all numbers as strings, so we return as String
                // and let the TryFrom implementation handle the parsing
                Ok(toasty_core::stmt::Value::String(n.clone()))
            }
            AttributeValue::Bool(b) => Ok(toasty_core::stmt::Value::Bool(*b)),
            AttributeValue::Null(_) => Ok(toasty_core::stmt::Value::Null),
            _ => Err(toasty::Error::msg(format!(
                "Unsupported DynamoDB AttributeValue type: {attr:?}"
            ))),
        }
    }
}
