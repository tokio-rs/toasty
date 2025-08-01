use toasty::driver::Capability;
use toasty::{db, Db};

use crate::Setup;

pub struct SetupSqlite;

impl SetupSqlite {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SetupSqlite {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Setup for SetupSqlite {
    async fn connect(&self, mut builder: db::Builder) -> toasty::Result<Db> {
        // SQLite uses in-memory databases, so no isolation needed
        builder.connect("sqlite::memory:").await
    }

    fn capability(&self) -> &Capability {
        &Capability::SQLITE
    }

    // SQLite uses in-memory databases, so no cleanup needed
    async fn cleanup_my_tables(&self) -> toasty::Result<()> {
        Ok(())
    }
}
