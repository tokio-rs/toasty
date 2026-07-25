use std::sync::Arc;

use toasty_core::driver::{ConnectContext, Connection as CoreConnection, Driver};
use tokio::sync::Mutex;

use super::{Connection, connection::ConnectionInner, pool::PoolConfig};

/// One request-local connection shared by every clone of a direct [`Db`](crate::Db).
pub(crate) struct Direct {
    driver: Box<dyn Driver>,
    connection: Arc<Mutex<Box<dyn CoreConnection>>>,
}

impl Direct {
    pub(crate) async fn new(driver: impl Driver, config: &PoolConfig) -> crate::Result<Self> {
        let mut cx = ConnectContext::default();
        cx.query_log = config.query_log;
        let connection = driver.connect(&cx).await?;

        Ok(Self {
            driver: Box::new(driver),
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub(crate) async fn get(&self, shared: Arc<super::Shared>) -> Connection {
        let inner = self.connection.clone().lock_owned().await;
        Connection {
            inner: ConnectionInner::Direct(Mutex::new(inner)),
            shared,
        }
    }

    pub(crate) fn driver(&self) -> &dyn Driver {
        self.driver.as_ref()
    }
}

impl std::fmt::Debug for Direct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Direct")
            .field("driver", &self.driver)
            .finish_non_exhaustive()
    }
}
