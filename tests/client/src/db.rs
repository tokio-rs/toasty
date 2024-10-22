use super::*;

pub struct SetupDynamoDb;

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    async fn setup(&self, schema: Schema) -> Db {
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        let prefix = format!("test_{}_", CNT.fetch_add(1, Relaxed));

        let driver = toasty_dynamodb::DynamoDB::from_env_with_prefix(&prefix)
            .await
            .unwrap();
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
        let driver = toasty_sqlite::Sqlite::new();
        let db = toasty::Db::new(schema, driver).await;
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql
    }
}
