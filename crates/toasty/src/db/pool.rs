use std::ops::{Deref, DerefMut};

use toasty_core::driver::{Capability, Driver};

use crate::db::{Connect, Connection};

#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    // TODO: Capability should just be constant for each driver and not require an active
    // connection to determine.
    capability: &'static Capability,
}

impl Pool {
    pub async fn new(driver: impl Driver) -> crate::Result<Self> {
        let inner = deadpool::managed::Pool::builder(Manager {
            driver: Box::new(driver),
        })
        .runtime(deadpool::Runtime::Tokio1)
        .build()?;
        let connection = match inner.get().await {
            Ok(connection) => connection,
            Err(err) => return Err(anyhow::anyhow!("failed to establish connection: {err}")),
        };
        Ok(Self {
            inner,
            capability: connection.capability(),
        })
    }

    pub async fn connect(url: &str) -> crate::Result<Self> {
        Self::new(Connect::new(url)?).await
    }

    pub async fn get(&self) -> crate::Result<ManagedConnection> {
        Ok(match self.inner.get().await {
            Ok(connection) => ManagedConnection { inner: connection },
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed to retrieve connection from pool: {err}"
                ))
            }
        })
    }

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

pub struct ManagedConnection {
    inner: deadpool::managed::Object<Manager>,
}

impl Deref for ManagedConnection {
    type Target = Box<dyn Connection>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ManagedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
