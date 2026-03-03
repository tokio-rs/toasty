mod builder;
mod connect;
mod pool;

pub use builder::Builder;
pub use connect::*;
pub use pool::*;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{engine::Engine, Executor, Result, Transaction, TransactionBuilder};

use toasty_core::{
    async_trait,
    driver::Driver,
    stmt::{Value, ValueStream},
    Schema,
};

use std::sync::Arc;

/// Shared state between all `Db` clones.
pub(crate) struct Shared {
    pub(crate) engine: Engine,
    pub(crate) pool: Pool,
}

/// Handle to a dedicated connection task.
///
/// When dropped, `in_tx` closes the channel, causing the background task to
/// finish processing remaining messages and exit gracefully.
struct ConnectionHandle {
    in_tx: mpsc::UnboundedSender<ConnectionOperation>,
    /// Kept so we can `.await` graceful shutdown in the future.
    #[allow(dead_code)]
    join_handle: JoinHandle<()>,
}

/// Operations sent to the connection task.
enum ConnectionOperation {
    /// Execute a statement (compile + run on the connection).
    ExecStatement {
        stmt: Box<toasty_core::stmt::Statement>,
        tx: oneshot::Sender<Result<ValueStream>>,
    },
    ExecOperation {
        operation: Operation,
        tx: oneshot::Sender<Result<Response>>,
    },
    /// Push schema to the database.
    PushSchema { tx: oneshot::Sender<Result<()>> },
}

/// A database handle. Each instance owns (or will lazily acquire) a dedicated
/// connection from the pool. Cloning produces a new handle that will acquire its
/// own connection on first use. Dropping the [`Db`] instance will release the database connection
/// back to the pool.
pub struct Db {
    shared: Arc<Shared>,
    connection: Option<ConnectionHandle>,
}

impl Clone for Db {
    fn clone(&self) -> Self {
        Db {
            shared: self.shared.clone(),
            // Cloned Db will acquire a new connection lazily.
            connection: None,
        }
    }
}

impl Db {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Lazily acquire a connection and spawn the background task.
    async fn connection(&mut self) -> Result<&ConnectionHandle> {
        if self.connection.is_none() {
            let mut connection = self.shared.pool.get().await?;
            let engine = self.shared.engine.clone();

            let (in_tx, mut in_rx) = mpsc::unbounded_channel::<ConnectionOperation>();

            let join_handle = tokio::spawn(async move {
                while let Some(op) = in_rx.recv().await {
                    match op {
                        ConnectionOperation::ExecStatement { stmt, tx } => {
                            match engine.exec(&mut connection, *stmt).await {
                                Ok(mut value_stream) => {
                                    let (row_tx, mut row_rx) =
                                        mpsc::unbounded_channel::<crate::Result<Value>>();

                                    let _ = tx.send(Ok(ValueStream::from_stream(
                                        async_stream::stream! {
                                            while let Some(res) = row_rx.recv().await {
                                                yield res
                                            }
                                        },
                                    )));

                                    while let Some(res) = value_stream.next().await {
                                        let _ = row_tx.send(res);
                                    }
                                }
                                Err(err) => {
                                    let _ = tx.send(Err(err));
                                }
                            }
                        }
                        ConnectionOperation::ExecOperation {
                            operation: stmt,
                            tx,
                        } => {
                            let result = connection.exec(&engine.schema, stmt).await;
                            let _ = tx.send(result);
                        }
                        ConnectionOperation::PushSchema { tx } => {
                            let result = connection.push_schema(&engine.schema).await;
                            let _ = tx.send(result);
                        }
                    }
                }
                // Channel closed → connection drops → returns to pool
            });

            self.connection = Some(ConnectionHandle { in_tx, join_handle });
        }
        Ok(self.connection.as_ref().unwrap())
    }

    pub(crate) async fn exec_operation(&mut self, operation: Operation) -> Result<Response> {
        let (tx, rx) = oneshot::channel();

        let conn = self.connection().await?;
        conn.in_tx
            .send(ConnectionOperation::ExecOperation { operation, tx })
            .unwrap();

        rx.await.unwrap()
    }

    pub fn transaction_builder(&mut self) -> TransactionBuilder<'_> {
        TransactionBuilder::new(self)
    }

    /// Creates tables and indices defined in the schema on the database.
    pub async fn push_schema(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let conn = self.connection().await?;
        conn.in_tx
            .send(ConnectionOperation::PushSchema { tx })
            .unwrap();
        rx.await.unwrap()
    }

    /// Drops the entire database and recreates an empty one without applying migrations.
    pub async fn reset_db(&self) -> Result<()> {
        self.shared.pool.driver().reset_db().await
    }

    pub fn driver(&self) -> &dyn Driver {
        self.shared.pool.driver()
    }

    pub fn schema(&self) -> &Arc<Schema> {
        &self.shared.engine.schema
    }

    pub fn capability(&self) -> &Capability {
        self.shared.engine.capability()
    }

    /// Returns a reference to the connection pool backing this handle.
    #[doc(hidden)]
    pub fn pool(&self) -> &Pool {
        &self.shared.pool
    }
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("engine", &self.shared.engine)
            .field("connected", &self.connection.is_some())
            .finish()
    }
}

#[async_trait]
impl Executor for Db {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        Transaction::begin(self).await
    }

    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<ValueStream> {
        let (tx, rx) = oneshot::channel();

        let conn = self.connection().await?;
        conn.in_tx
            .send(ConnectionOperation::ExecStatement {
                stmt: Box::new(stmt),
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    fn schema(&mut self) -> &Arc<Schema> {
        Db::schema(self)
    }
}
