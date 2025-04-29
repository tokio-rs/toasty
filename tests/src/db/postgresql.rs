use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupPostgreSQL;

#[async_trait::async_trait]
impl Setup for SetupPostgreSQL {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

        static CNT: AtomicUsize = AtomicUsize::new(0);

        thread_local! {
            pub static PREFIX: String = format!("test_{}_", CNT.fetch_add(1, Relaxed));
        }

        let prefix = PREFIX.with(|k| k.clone());

        let url = std::env::var("TOASTY_TEST_POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/toasty_test".to_string());

        builder.table_name_prefix(&prefix).connect(&url).await
    }

    fn capability(&self) -> &Capability {
        &Capability::POSTGRESQL
    }
}
