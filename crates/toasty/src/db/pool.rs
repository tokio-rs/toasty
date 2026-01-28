//! Connection pooling for database connections.

use std::ops::{Deref, DerefMut};

pub use deadpool::managed::Timeouts;
use toasty_core::driver::{Capability, Driver};

use crate::db::{Connect, Connection};

/// Get the default maximum size of a pool, which is `cpu_core_count * 2`
/// including logical cores (Hyper-Threading).
fn get_default_pool_max_size() -> usize {
    deadpool::managed::PoolConfig::default().max_size
}

/// Configuration for connection pool behavior.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_size: usize,
    pub timeouts: Timeouts,
}

impl PoolConfig {
    /// Creates a new pool configuration with default settings.
    pub fn new() -> Self {
        Self {
            max_size: get_default_pool_max_size(),
            timeouts: Default::default(),
        }
    }
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A connection pool that manages database connections.
#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    // TODO: Capability should just be constant for each driver and not require an active
    // connection to determine.
    capability: &'static Capability,
}

impl Pool {
    /// Creates a new connection pool from the given driver.
    pub async fn new(driver: impl Driver) -> crate::Result<Self> {
        let max_connections = driver.max_connections();
        let mut builder = deadpool::managed::Pool::builder(Manager {
            driver: Box::new(driver),
        })
        .runtime(deadpool::Runtime::Tokio1);

        if let Some(max_connections) = max_connections {
            builder = builder.max_size(max_connections);
        }

        let inner = builder
            .build()
            .map_err(toasty_core::Error::connection_pool)?;

        let connection = inner
            .get()
            .await
            .map_err(toasty_core::Error::connection_pool)?;
        Ok(Self {
            inner,
            capability: connection.capability(),
        })
    }

    /// Creates a new connection pool from a connection URL.
    pub async fn connect(url: &str) -> crate::Result<Self> {
        Self::new(Connect::new(url)?).await
    }

    /// Retrieves a connection from the pool.
    pub async fn get(&self) -> crate::Result<PoolConnection> {
        let connection = self
            .inner
            .get()
            .await
            .map_err(toasty_core::Error::connection_pool)?;
        Ok(PoolConnection { inner: connection })
    }

    /// Returns the database driver's capabilities.
    pub fn capability(&self) -> &Capability {
        self.capability
    }
}

#[derive(Debug)]
struct Manager {
    driver: Box<dyn Driver>,
}

impl deadpool::managed::Manager for Manager {
    type Type = Box<dyn Connection>;
    type Error = crate::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        self.driver.connect().await
    }

    async fn recycle(
        &self,
        _obj: &mut Self::Type,
        _metrics: &deadpool::managed::Metrics,
    ) -> deadpool::managed::RecycleResult<Self::Error> {
        Ok(())
    }
}

/// A connection retrieved from a pool.
///
/// When dropped, the connection is returned to the pool for reuse.
pub struct PoolConnection {
    inner: deadpool::managed::Object<Manager>,
}

impl Deref for PoolConnection {
    type Target = Box<dyn Connection>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PoolConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
