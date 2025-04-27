use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupDynamoDb;

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    async fn setup(&self, mut builder: db::Builder) -> Db {
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        thread_local! {
            pub static PREFIX: String = format!("test_{}_", CNT.fetch_add(1, Relaxed));
        }

        let prefix = PREFIX.with(|k| k.clone());

        let url =
            std::env::var("TOASTY_TEST_DYNAMODB_URL").unwrap_or_else(|_| "dynamodb://".to_string());

        let db = builder
            .table_name_prefix(&prefix)
            .connect(&url)
            .await
            .unwrap();

        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::DYNAMODB
    }
}
