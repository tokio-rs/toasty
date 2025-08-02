use std::collections::HashMap;
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

    async fn get_raw_column_value<T>(
        &self,
        _table: &str,
        _column: &str,
        _filter: HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: TryFrom<toasty_core::stmt::Value, Error = toasty_core::Error>,
    {
        // For SQLite, we'll need to connect to the same in-memory database
        // This is tricky since each connection gets its own in-memory database
        // For now, return an error indicating this limitation
        Err(toasty::Error::msg(
            "SQLite in-memory database raw value access not yet implemented",
        ))
    }
}
