#![cfg(feature = "dynamodb")]

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::{config::Credentials, Client};
use tokio::sync::OnceCell;

struct DynamoDbSetup {
    client: OnceCell<Client>,
}

impl DynamoDbSetup {
    fn new() -> Self {
        Self {
            client: OnceCell::new(),
        }
    }

    async fn get_client(&self) -> &Client {
        self.client
            .get_or_init(|| async {
                let config = aws_config::defaults(BehaviorVersion::latest())
                    .region("us-east-1")
                    .credentials_provider(Credentials::for_tests())
                    .endpoint_url("http://localhost:8000")
                    .load()
                    .await;

                Client::new(&config)
            })
            .await
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for DynamoDbSetup {
    fn driver(&self) -> Box<dyn toasty::driver::Driver> {
        let url =
            std::env::var("TOASTY_TEST_DYNAMODB_URL").unwrap_or_else(|_| "dynamodb://".to_string());
        Box::new(toasty::db::Connect::new(&url).expect("Failed to create DynamoDB driver"))
    }

    async fn delete_table(&self, name: &str) {
        let client = self.get_client().await;

        // Delete the table - ignore errors if it doesn't exist
        let _ = client.delete_table().table_name(name).send().await;
    }
}

// Generate all driver tests (DynamoDB doesn't support auto_increment, bigdecimal, or decimal)
toasty_driver_integration_suite::generate_driver_tests!(DynamoDbSetup::new(),
    auto_increment: false,
    bigdecimal_implemented: false,
    decimal_arbitrary_precision: false,
    native_decimal: false,
    native_varchar: false,
    native_timestamp: false,
    native_date: false,
    native_time: false,
    native_datetime: false,
);
