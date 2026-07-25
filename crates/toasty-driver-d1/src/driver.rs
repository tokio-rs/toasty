use std::{borrow::Cow, fmt, sync::Mutex};

use async_trait::async_trait;
use toasty_core::{
    driver::{Capability, ConnectContext, ConnectionStrategy, Driver},
    schema::{db::Migration, diff},
};
use worker::{D1Database, send::SendWrapper};

use crate::{connection::Connection, error, migration};

/// A request-local Cloudflare D1 driver.
pub struct D1 {
    binding_name: String,
    database: Mutex<Option<SendWrapper<D1Database>>>,
}

impl D1 {
    /// Creates a driver from a Worker D1 binding.
    pub fn new(binding_name: impl Into<String>, database: D1Database) -> Self {
        Self {
            binding_name: binding_name.into(),
            database: Mutex::new(Some(SendWrapper::new(database))),
        }
    }
}

impl fmt::Debug for D1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("D1")
            .field("binding_name", &self.binding_name)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Driver for D1 {
    fn url(&self) -> Cow<'_, str> {
        Cow::Owned(format!("d1:{}", self.binding_name))
    }

    fn capability(&self) -> &'static Capability {
        &Capability::D1
    }

    fn connection_strategy(&self) -> ConnectionStrategy {
        ConnectionStrategy::Direct
    }

    async fn connect(
        &self,
        cx: &ConnectContext,
    ) -> toasty_core::Result<Box<dyn toasty_core::Connection>> {
        let database = self
            .database
            .lock()
            .map_err(|_| {
                toasty_core::Error::invalid_driver_configuration(
                    "D1 binding ownership lock was poisoned",
                )
            })?
            .take()
            .ok_or_else(|| {
                toasty_core::Error::invalid_driver_configuration(
                    "D1 drivers can create only one direct connection",
                )
            })?;

        Ok(Box::new(Connection::new(database, cx.query_log)))
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        migration::generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        Err(error::unsupported("reset database"))
    }
}
