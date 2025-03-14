use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupDynamoDb;

#[async_trait::async_trait]
impl Setup for SetupDynamoDb {
    async fn setup(&self, mut builder: db::Builder) -> Db {
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        let prefix = format!("test_{}_", CNT.fetch_add(1, Relaxed));

        let db = builder
            .table_name_prefix(&prefix)
            .connect("dynamodb://")
            .await
            .unwrap();

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
