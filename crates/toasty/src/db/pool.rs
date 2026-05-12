//! Connection pooling for database connections.

use deadpool::managed::Timeouts;
use std::sync::Arc;
use toasty_core::driver::{Capability, Driver};

use super::connection_task::ConnectionHandle;
use crate::engine::Engine;

/// Get the default maximum size of a pool, which is `cpu_core_count * 2`
/// including logical cores (Hyper-Threading).
fn get_default_pool_max_size() -> usize {
    deadpool::managed::PoolConfig::default().max_size
}

/// Configuration for connection pool behavior.
#[derive(Debug, Clone)]
pub(crate) struct PoolConfig {
    pub(crate) max_size: usize,
    pub(crate) timeouts: Timeouts,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: get_default_pool_max_size(),
            timeouts: Default::default(),
        }
    }
}

/// A connection pool that manages database connections with background tasks.
#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    capability: &'static Capability,
}

impl Pool {
    /// Creates a new connection pool from the given driver, engine, and
    /// configuration.
    pub(crate) fn new(
        driver: impl Driver,
        engine: Engine,
        config: PoolConfig,
    ) -> crate::Result<Self> {
        let capability = driver.capability();
        let driver_cap = driver.max_connections();

        let effective_max = match driver_cap {
            Some(cap) if cap < config.max_size => {
                tracing::warn!(
                    requested = config.max_size,
                    cap,
                    "driver caps max pool size below the requested value; using driver cap"
                );
                cap
            }
            _ => config.max_size,
        };

        let inner = deadpool::managed::Pool::builder(Manager {
            driver: Box::new(driver),
            engine,
        })
        .runtime(deadpool::Runtime::Tokio1)
        .max_size(effective_max)
        .timeouts(config.timeouts)
        .build()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to build connection pool");
            toasty_core::Error::connection_pool(e)
        })?;

        Ok(Self { inner, capability })
    }

    /// Retrieves a connection from the pool.
    pub(crate) async fn get(&self, shared: Arc<super::Shared>) -> crate::Result<super::Connection> {
        let connection = self.inner.get().await.map_err(|e| {
            tracing::error!(error = %e, "failed to acquire connection from pool");
            toasty_core::Error::connection_pool(e)
        })?;
        Ok(super::Connection {
            inner: connection,
            shared,
        })
    }

    /// Returns the database driver this pool uses to create connections.
    pub fn driver(&self) -> &dyn Driver {
        self.inner.manager().driver.as_ref()
    }

    /// Returns the database driver's capabilities.
    pub fn capability(&self) -> &'static Capability {
        self.capability
    }

    /// Returns the current status of the pool, including the number of
    /// connections, how many are available, and how many waiters are queued.
    pub fn status(&self) -> PoolStatus {
        let s = self.inner.status();
        PoolStatus {
            max_size: s.max_size,
            size: s.size,
            available: s.available,
            waiting: s.waiting,
        }
    }
}

pub(super) struct Manager {
    driver: Box<dyn Driver>,
    engine: Engine,
}

impl std::fmt::Debug for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Manager")
            .field("driver", &self.driver)
            .finish()
    }
}

impl deadpool::managed::Manager for Manager {
    type Type = ConnectionHandle;
    type Error = crate::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        tracing::debug!("creating new pooled connection");
        let connection = self.driver.connect().await.inspect_err(|e| {
            tracing::error!(error = %e, "failed to create database connection");
        })?;
        Ok(ConnectionHandle::spawn(connection, self.engine.clone()))
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &deadpool::managed::Metrics,
    ) -> deadpool::managed::RecycleResult<Self::Error> {
        if obj.in_tx.is_closed() || obj.is_finished() {
            tracing::debug!("discarding dead pooled connection");
            return Err(deadpool::managed::RecycleError::message(
                "background task is no longer running",
            ));
        }
        tracing::trace!("recycling pooled connection");
        Ok(())
    }
}

/// Snapshot of the pool's current state.
#[derive(Clone, Copy, Debug)]
pub struct PoolStatus {
    /// The maximum number of connections the pool will manage.
    pub max_size: usize,

    /// The current number of connections (both in-use and idle).
    pub size: usize,

    /// The number of idle connections ready to be checked out.
    pub available: usize,

    /// The number of tasks waiting for a connection to become available.
    pub waiting: usize,
}
