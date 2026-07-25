use std::sync::Arc;

use toasty_core::driver::Driver;

use super::{Connection, Pool, direct::Direct, pool::PoolConfig};
use crate::engine::Engine;

#[derive(Debug)]
pub(crate) enum ConnectionSource {
    Pooled(Pool),
    Direct(Direct),
}

impl ConnectionSource {
    pub(crate) fn pooled(
        driver: impl Driver,
        engine: Engine,
        config: PoolConfig,
    ) -> crate::Result<Self> {
        Ok(Self::Pooled(Pool::new(driver, engine, config)?))
    }

    pub(crate) async fn direct(driver: impl Driver, config: &PoolConfig) -> crate::Result<Self> {
        Ok(Self::Direct(Direct::new(driver, config).await?))
    }

    pub(crate) async fn get(&self, shared: Arc<super::Shared>) -> crate::Result<Connection> {
        match self {
            Self::Pooled(pool) => pool.get(shared).await,
            Self::Direct(direct) => Ok(direct.get(shared).await),
        }
    }

    pub(crate) fn driver(&self) -> &dyn Driver {
        match self {
            Self::Pooled(pool) => pool.driver(),
            Self::Direct(direct) => direct.driver(),
        }
    }

    pub(crate) fn pool(&self) -> Option<&Pool> {
        match self {
            Self::Pooled(pool) => Some(pool),
            Self::Direct(_) => None,
        }
    }
}
