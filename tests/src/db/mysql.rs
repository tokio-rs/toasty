use toasty::driver::{Capability, CapabilitySql};
use toasty::{db, Db};

use crate::Setup;

pub struct SetupMySQL;

#[async_trait::async_trait]
impl Setup for SetupMySQL {
    async fn setup(&self, mut builder: db::Builder) -> Db {
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        thread_local! {
            pub static PREFIX: String = format!("test_{}_", CNT.fetch_add(1, Relaxed));
        }

        let prefix = PREFIX.with(|k| k.clone());

        let url = std::env::var("TOASTY_TEST_MYSQL_URL")
            .unwrap_or_else(|_| "mysql://localhost:3306/toasty_test".to_string());

        let db = builder
            .table_name_prefix(&prefix)
            .connect(&url)
            .await
            .unwrap();

        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql(CapabilitySql {
            cte_with_update: false,
        })
    }
}
