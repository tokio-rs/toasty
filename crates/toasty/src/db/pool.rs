//! Connection pooling for database connections.

pub use deadpool::managed::Timeouts;
use std::sync::Arc;
use toasty_core::driver::{Capability, Driver, Rows};
use toasty_core::stmt::Value;
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
    /// Maximum number of connections the pool will maintain.
    pub max_size: usize,
    /// Timeout settings for acquiring a connection from the pool.
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
        tx: oneshot::Sender<crate::Result<toasty_core::driver::Response>>,
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

        let inner = builder.build().map_err(|e| {
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
        let mut connection = match self.driver.connect().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::error!(error = %e, "failed to create database connection");
                return Err(e);
            }
        };
        let engine = self.engine.clone();

        let (in_tx, mut in_rx) = mpsc::unbounded_channel::<ConnectionOperation>();

        let join_handle = tokio::spawn(async move {
            while let Some(op) = in_rx.recv().await {
                match op {
                    ConnectionOperation::ExecStatement {
                        stmt,
                        in_transaction,
                        tx,
                    } => {
                        let single = stmt.is_single();
                        let result = async {
                            let mut response = engine
                                .exec_with_metadata(&mut *connection, *stmt, in_transaction)
                                .await?;
                            response.values.buffer().await?;

                            if single {
                                let Rows::Value(Value::List(mut items)) = response.values else {
                                    unreachable!()
                                };
                                assert!(
                                    items.len() <= 1,
                                    "expected at most 1 row for single statement, got {}",
                                    items.len()
                                );
                                response.values = Rows::Value(items.pop().unwrap_or(Value::Null));
                            }

                            Ok(response)
                        }
                        .await;

                        let _ = tx.send(result);
                    }
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
