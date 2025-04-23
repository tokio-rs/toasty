use toasty::driver::{Capability, CapabilitySql};
use toasty::{db, Db};

use crate::Setup;

pub struct SetupSqlite;

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn setup(&self, mut builder: db::Builder) -> Db {
        let db = builder.connect("sqlite::memory:").await.unwrap();
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql(CapabilitySql {
            cte_with_update: false,
            select_for_update: false,
        })
    }
}
