use super::*;

pub struct SetupDynamoDb;

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    async fn setup(&self, schema: Schema) -> Db {
        use aws_config::BehaviorVersion;
        use aws_sdk_dynamodb::{config::Credentials, Client};
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        let prefix = format!("test_{}_", CNT.fetch_add(1, Relaxed));

        let mut sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .credentials_provider(Credentials::for_tests());

        if std::env::var("AWS_DEFAULT_REGION").is_err() {
            sdk_config = sdk_config.region("local");
        }

        if std::env::var("AWS_ENDPOINT_URL_DYNAMODB").is_err() {
            sdk_config = sdk_config.endpoint_url("http://localhost:8000");
        }

        let client = Client::new(&sdk_config.load().await);

        let driver = toasty_dynamodb::DynamoDB::new(client, Some(prefix));
        let db = toasty::Db::new(schema, driver).await;
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        use toasty_core::driver::capability::KeyValue;

        &Capability::KeyValue(KeyValue {
            primary_key_ne_predicate: false,
        })
    }
}

pub struct SetupSqlite;

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn setup(&self, schema: Schema) -> Db {
        let driver = toasty_sqlite::Sqlite::in_memory();
        let db = toasty::Db::new(schema, driver).await;
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql
    }
}
