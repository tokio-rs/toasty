use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupSqlite;

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        builder.connect("sqlite::memory:").await
    }

    fn capability(&self) -> &Capability {
        &Capability::SQLITE
    }
}
