use std::ops::{Deref, DerefMut};

use toasty_core::{driver::Capability, Driver};

use crate::db::connection::Connection;

#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    // TODO: Capability should just be constant for each driver and not require an active
    // connection to determine.
    capability: &'static Capability,
}

impl Pool {
    pub async fn connect(url: &str) -> crate::Result<Self> {
        let inner = deadpool::managed::Pool::builder(Manager {
            url: url.to_string(),
        })
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
    url: String,
}

impl deadpool::managed::Manager for Manager {
    type Type = Connection;
    type Error = crate::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        Connection::connect(&self.url).await
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
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ManagedConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
