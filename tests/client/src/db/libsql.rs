use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupLibSQL;

#[async_trait::async_trait]
impl Setup for SetupLibSQL {
    async fn setup(&self, mut builder: db::Builder) -> Db {
        let db = builder.connect("libsql::memory:").await.unwrap();
        db.reset_db().await.unwrap();
        db
    }

    fn capability(&self) -> &Capability {
        &Capability::Sql
    }
}
