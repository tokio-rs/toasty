#![cfg(feature = "dynamodb")]

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use std::sync::OnceLock;
use toasty_driver_dynamodb::DynamoDb;

struct DynamoDbSetup {
    client: OnceLock<Client>,
}

impl DynamoDbSetup {
    fn new() -> Self {
        Self {
            client: OnceLock::new(),
        }
    }

    fn get_client(&self) -> &Client {
        self.client.get_or_init(|| {
            // Set default AWS environment variables if not already set
            if std::env::var("AWS_REGION").is_err() {
                std::env::set_var("AWS_REGION", "us-east-1");
            }
            if std::env::var("AWS_ENDPOINT_URL_DYNAMODB").is_err() {
                std::env::set_var("AWS_ENDPOINT_URL_DYNAMODB", "http://localhost:8000");
            }
            if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
                std::env::set_var("AWS_ACCESS_KEY_ID", "test");
            }
            if std::env::var("AWS_SECRET_ACCESS_KEY").is_err() {
                std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");
            }

            // Spawn a thread to handle async AWS SDK initialization
            std::thread::spawn(|| {
                tokio::runtime::Runtime::new()
                    .expect("Failed to create tokio runtime")
                    .block_on(async {
                        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
                        Client::new(&config)
                    })
            })
            .join()
            .expect("Failed to join client initialization thread")
        })
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for DynamoDbSetup {
    fn driver(&self) -> Box<dyn toasty::driver::Driver> {
        let client = self.get_client();
        Box::new(DynamoDb::new("dynamodb://".to_string(), client.clone()))
    }

    async fn delete_table(&self, name: &str) {
        let client = self.get_client();

        // Delete the table - ignore errors if it doesn't exist
        let _ = client.delete_table().table_name(name).send().await;
    }
}

// Generate all driver tests (DynamoDB doesn't support auto_increment, bigdecimal, or decimal)
toasty_driver_integration_suite::generate_driver_tests!(DynamoDbSetup::new(),
    sql: false,
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
