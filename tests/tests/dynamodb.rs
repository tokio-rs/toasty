#![cfg(feature = "dynamodb")]

use aws_config::BehaviorVersion;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::config::Credentials;
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
            // Spawn a thread to handle async AWS SDK initialization
            std::thread::spawn(|| {
                tokio::runtime::Runtime::new()
                    .expect("Failed to create tokio runtime")
                    .block_on(async {
                        // Configure for DDB Local, if configs are not already provided.
                        // We can point tests to real DDB with a couple of environment variables.
                        let region_provider =
                            RegionProviderChain::default_provider().or_else("us-east-1");
                        let mut config_loader =
                            aws_config::defaults(BehaviorVersion::latest()).region(region_provider);
                        if std::env::var("AWS_ENDPOINT_URL_DYNAMODB").is_err() {
                            config_loader = config_loader.endpoint_url("http://localhost:8000");
                        }
                        if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
                            config_loader =
                                config_loader.credentials_provider(Credentials::for_tests());
                        }
                        let config = config_loader.load().await;
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
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
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
    backward_pagination: false,
);
