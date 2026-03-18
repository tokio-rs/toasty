//! Connection pooling for database connections.

pub use deadpool::managed::Timeouts;
use toasty_core::driver::{Capability, Driver};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::engine::Engine;

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

/// Handle to a dedicated connection task.
///
/// When dropped, `in_tx` closes the channel, causing the background task to
/// finish processing remaining messages and exit gracefully.
pub(crate) struct ConnectionHandle {
    pub(crate) in_tx: mpsc::UnboundedSender<ConnectionOperation>,
    join_handle: JoinHandle<()>,
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("channel_closed", &self.in_tx.is_closed())
            .field("task_finished", &self.join_handle.is_finished())
            .finish()
    }
}

/// Operations sent to the connection task.
pub(crate) enum ConnectionOperation {
    /// Execute a statement (compile + run on the connection).
    ExecStatement {
        stmt: Box<toasty_core::stmt::Statement>,
        in_transaction: bool,
        tx: oneshot::Sender<crate::Result<toasty_core::stmt::ValueStream>>,
    },
    ExecOperation {
        operation: Box<toasty_core::driver::operation::Operation>,
        tx: oneshot::Sender<crate::Result<toasty_core::driver::Response>>,
    },
    /// Push schema to the database.
    PushSchema {
        tx: oneshot::Sender<crate::Result<()>>,
    },
}

/// A connection pool that manages database connections with background tasks.
#[derive(Debug)]
pub struct Pool {
    inner: deadpool::managed::Pool<Manager>,
    capability: &'static Capability,
}

impl Pool {
    /// Creates a new connection pool from the given driver and engine.
    pub(crate) fn new(driver: impl Driver, engine: Engine) -> crate::Result<Self> {
        let capability = driver.capability();
        let max_connections = driver.max_connections();

        let mut builder = deadpool::managed::Pool::builder(Manager {
            driver: Box::new(driver),
            engine,
        })
        .runtime(deadpool::Runtime::Tokio1);

        if let Some(max_connections) = max_connections {
            builder = builder.max_size(max_connections);
        }

        let inner = builder
            .build()
            .map_err(toasty_core::Error::connection_pool)?;

        Ok(Self { inner, capability })
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

struct Manager {
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
        let mut connection = self.driver.connect().await?;
        let engine = self.engine.clone();

        let (in_tx, mut in_rx) = mpsc::unbounded_channel::<ConnectionOperation>();

        let join_handle = tokio::spawn(async move {
            while let Some(op) = in_rx.recv().await {
                match op {
                    ConnectionOperation::ExecStatement {
                        stmt,
                        in_transaction,
                        tx,
                    } => match engine.exec(&mut *connection, *stmt, in_transaction).await {
                        Ok(mut value_stream) => {
                            let (row_tx, mut row_rx) = mpsc::unbounded_channel::<
                                crate::Result<toasty_core::stmt::Value>,
                            >();
                            let cursor = value_stream.take_cursor();
                            eprintln!("Building ValueStream::from_stream: {:?}", cursor);
                            let _ = tx.send(Ok(toasty_core::stmt::ValueStream::from_stream(
                                async_stream::stream! {
                                    while let Some(res) = row_rx.recv().await {
                                        yield res
                                    }
                                },
                            )
                            .with_cursor(cursor)));

                            while let Some(res) = value_stream.next().await {
                                let _ = row_tx.send(res);
                            }
                        }
                        Err(err) => {
                            let _ = tx.send(Err(err));
                        }
                    },
                    ConnectionOperation::ExecOperation { operation, tx } => {
                        let result = connection.exec(&engine.schema, *operation).await;
                        let _ = tx.send(result);
                    }
                    ConnectionOperation::PushSchema { tx } => {
                        let result = connection.push_schema(&engine.schema).await;
                        let _ = tx.send(result);
                    }
                }
            }
        });

        Ok(ConnectionHandle { in_tx, join_handle })
    }

    async fn recycle(
        &self,
        obj: &mut Self::Type,
        _metrics: &deadpool::managed::Metrics,
    ) -> deadpool::managed::RecycleResult<Self::Error> {
        if obj.in_tx.is_closed() || obj.join_handle.is_finished() {
            return Err(deadpool::managed::RecycleError::message(
                "background task is no longer running",
            ));
        }
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

/// A connection retrieved from a pool.
///
/// When dropped, the connection is returned to the pool for reuse.
pub struct PoolConnection {
    inner: deadpool::managed::Object<Manager>,
}

impl PoolConnection {
    /// Access the underlying connection handle.
    pub(crate) fn handle(&self) -> &ConnectionHandle {
        &self.inner
    }
}
