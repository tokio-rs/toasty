#![cfg(feature = "dynamodb")]

use aws_config::BehaviorVersion;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::config::Credentials;
use std::sync::{Arc, OnceLock};
use toasty::Result;
use toasty::models;
use toasty_driver_dynamodb::DynamoDb;
use toasty_driver_integration_suite::Test;

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
    native_array: false,
    vec_scalar: true,
    document_collections: false,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    backward_pagination: false,
    test_connection_pool: false,
);

// ─────────────────────────────────────────────────────────────────────────────
// DynamoDB 1 MB response boundary tests.
//
// These tests prove the paging loop works correctly when the result set spans
// DynamoDB's 1 MB response boundary. DynamoDB's Query API returns at most 1 MB
// of data per call and sets `LastEvaluatedKey` when there are more results.
// With ~10 KB items, ~100 items ≈ 1 MB, so seeding 200 items and querying with
// `.limit(150)` forces at least 2 DynamoDB API calls, exercising the pagination
// loop in the driver.
//
// IMPORTANT: The `payload` field is intentionally large (10,000 bytes). Do NOT
// reduce its size. The tests depend on the payload being large enough to push
// each batch of ~100 items past the 1 MB boundary so that at least two DynamoDB
// API calls are required to satisfy the limit.
// ─────────────────────────────────────────────────────────────────────────────

// ── Base table tests (composite partition + sort key) ────────────────────────

#[test]
fn limit_spans_page_boundary() {
    let mut test = Test::new(Arc::new(DynamoDbSetup::new()));
    test.run(async move |t: &mut Test| -> Result<()> {
        #[derive(Debug, toasty::Model)]
        #[key(partition = kind, local = seq)]
        struct Item {
            kind: String,
            seq: i64,
            payload: String,
        }

        let mut db = t.setup_db(models!(Item)).await;

        let payload = "x".repeat(10_000);
        for i in 0..200_i64 {
            toasty::create!(Item {
                kind: "boundary",
                seq: i,
                payload: payload.clone(),
            })
            .exec(&mut db)
            .await?;
        }

        let items: Vec<_> = Item::filter_by_kind("boundary")
            .order_by(Item::fields().seq().asc())
            .limit(150)
            .exec(&mut db)
            .await?;

        assert_eq!(items.len(), 150);
        assert_eq!(items[0].seq, 0);
        assert_eq!(items[149].seq, 149);

        Ok(())
    });
}

#[test]
fn limit_offset_spans_page_boundary() {
    let mut test = Test::new(Arc::new(DynamoDbSetup::new()));
    test.run(async move |t: &mut Test| -> Result<()> {
        #[derive(Debug, toasty::Model)]
        #[key(partition = kind, local = seq)]
        struct Item {
            kind: String,
            seq: i64,
            payload: String,
        }

        let mut db = t.setup_db(models!(Item)).await;

        let payload = "x".repeat(10_000);
        for i in 0..200_i64 {
            toasty::create!(Item {
                kind: "boundary",
                seq: i,
                payload: payload.clone(),
            })
            .exec(&mut db)
            .await?;
        }

        let items: Vec<_> = Item::filter_by_kind("boundary")
            .order_by(Item::fields().seq().asc())
            .limit(100)
            .offset(50)
            .exec(&mut db)
            .await?;

        assert_eq!(items.len(), 100);
        assert_eq!(items[0].seq, 50);
        assert_eq!(items[99].seq, 149);

        Ok(())
    });
}

/// No-limit query across a 1 MB boundary returns **all** rows.
///
/// With ~10 KB items and 200 rows (~2 MB total), a single DynamoDB `Query`
/// call is capped at 1 MB and returns only ~100 rows. The driver must follow
/// `LastEvaluatedKey` and keep querying until all results are returned.
#[test]
fn no_limit_spans_page_boundary() {
    let mut test = Test::new(Arc::new(DynamoDbSetup::new()));
    test.run(async move |t: &mut Test| -> Result<()> {
        #[derive(Debug, toasty::Model)]
        #[key(partition = kind, local = seq)]
        struct Item {
            kind: String,
            seq: i64,
            payload: String,
        }

        let mut db = t.setup_db(models!(Item)).await;

        let payload = "x".repeat(10_000);
        for i in 0..200_i64 {
            toasty::create!(Item {
                kind: "boundary",
                seq: i,
                payload: payload.clone(),
            })
            .exec(&mut db)
            .await?;
        }

        let items: Vec<_> = Item::filter_by_kind("boundary").exec(&mut db).await?;

        assert_eq!(items.len(), 200);

        Ok(())
    });
}

// ── GSI tests (non-unique index on a UUID-keyed model) ───────────────────────

#[test]
fn limit_spans_page_boundary_gsi() {
    let mut test = Test::new(Arc::new(DynamoDbSetup::new()));
    test.run(async move |t: &mut Test| -> Result<()> {
        #[derive(Debug, toasty::Model)]
        struct GsiItem {
            #[key]
            #[auto]
            id: uuid::Uuid,

            #[index]
            category: String,

            seq: i64,
            payload: String,
        }

        let mut db = t.setup_db(models!(GsiItem)).await;

        let payload = "x".repeat(10_000);
        for i in 0..200_i64 {
            toasty::create!(GsiItem {
                category: "boundary",
                seq: i,
                payload: payload.clone(),
            })
            .exec(&mut db)
            .await?;
        }

        // DDB GSI ordering is only guaranteed on the GSI sort key; seq is not
        // the sort key here, so only assert the count.
        let items: Vec<_> = GsiItem::filter_by_category("boundary")
            .limit(150)
            .exec(&mut db)
            .await?;

        assert_eq!(items.len(), 150);

        Ok(())
    });
}

#[test]
fn limit_offset_spans_page_boundary_gsi() {
    let mut test = Test::new(Arc::new(DynamoDbSetup::new()));
    test.run(async move |t: &mut Test| -> Result<()> {
        #[derive(Debug, toasty::Model)]
        struct GsiItem {
            #[key]
            #[auto]
            id: uuid::Uuid,

            #[index]
            category: String,

            seq: i64,
            payload: String,
        }

        let mut db = t.setup_db(models!(GsiItem)).await;

        let payload = "x".repeat(10_000);
        for i in 0..200_i64 {
            toasty::create!(GsiItem {
                category: "boundary",
                seq: i,
                payload: payload.clone(),
            })
            .exec(&mut db)
            .await?;
        }

        // DDB GSI ordering is only guaranteed on the GSI sort key; seq is not
        // the sort key here, so only assert the count.
        let items: Vec<_> = GsiItem::filter_by_category("boundary")
            .limit(100)
            .offset(50)
            .exec(&mut db)
            .await?;

        assert_eq!(items.len(), 100);

        Ok(())
    });
}
