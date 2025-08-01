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
            .map_err(|e| toasty::Error::message(format!("DynamoDB cleanup failed: {}", e)))
    }
}

async fn cleanup_dynamodb_tables(
    isolation: &TestIsolation,
) -> Result<(), Box<dyn std::error::Error>> {
    // For now, we'll implement a basic cleanup that doesn't require additional AWS dependencies
    // This can be enhanced later when we have access to the AWS SDK through workspace dependencies

    // TODO: Implement DynamoDB table cleanup using the AWS SDK
    // This would involve:
    // 1. List all tables
    // 2. Filter for tables with our prefix
    // 3. Delete matching tables

    let _ = isolation; // Suppress unused variable warning

    // For now, return Ok since DynamoDB tables are typically short-lived in test environments
    // and the unique prefixes prevent conflicts which was the main goal
    Ok(())
}
