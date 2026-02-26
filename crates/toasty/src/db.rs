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

use crate::{engine::Engine, stmt, Cursor, Model, Result, Statement};

use toasty_core::{
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
struct ConnHandle {
    in_tx: mpsc::UnboundedSender<ConnOp>,
    /// Kept so we can `.await` graceful shutdown in the future.
    #[allow(dead_code)]
    join_handle: JoinHandle<()>,
}

/// Operations sent to the connection task.
enum ConnOp {
    /// Execute a statement (compile + run on the connection).
    Exec {
        stmt: toasty_core::stmt::Statement,
        tx: oneshot::Sender<Result<ValueStream>>,
    },
    /// Push schema to the database.
    PushSchema { tx: oneshot::Sender<Result<()>> },
}

/// A database handle. Each instance owns (or will lazily acquire) a dedicated
/// connection from the pool. Cloning produces a new handle that will acquire its
/// own connection on first use.
#[derive(Clone)]
pub struct Db {
    shared: Arc<Shared>,
    conn: Option<Arc<ConnHandle>>,
}

impl Db {
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Lazily acquire a connection and spawn the background task.
    async fn connection(&mut self) -> Result<&ConnHandle> {
        if self.conn.is_none() {
            let mut connection = self.shared.pool.get().await?;
            let engine = self.shared.engine.clone();

            let (in_tx, mut in_rx) = mpsc::unbounded_channel::<ConnOp>();

            let join_handle = tokio::spawn(async move {
                while let Some(op) = in_rx.recv().await {
                    match op {
                        ConnOp::Exec { stmt, tx } => {
                            match engine.exec(&mut connection, stmt).await {
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
                        ConnOp::PushSchema { tx } => {
                            let result = connection
                                .push_schema(&engine.schema.db)
                                .await
                                .map_err(Into::into);
                            let _ = tx.send(result);
                        }
                    }
                }
                // Channel closed → connection drops → returns to pool
            });

            self.conn = Some(Arc::new(ConnHandle { in_tx, join_handle }));
        }
        Ok(self.conn.as_ref().unwrap())
    }

    /// Execute a query, returning all matching records
    pub async fn all<M: Model>(&mut self, query: stmt::Select<M>) -> Result<Cursor<M>> {
        let records = self.exec(query.into()).await?;
        Ok(Cursor::new(self.shared.engine.schema.clone(), records))
    }

    pub async fn first<M: Model>(&mut self, query: stmt::Select<M>) -> Result<Option<M>> {
        let mut res = self.all(query).await?;
        match res.next().await {
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    pub async fn get<M: Model>(&mut self, query: stmt::Select<M>) -> Result<M> {
        let mut res = self.all(query).await?;

        match res.next().await {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => Err(toasty_core::Error::record_not_found(
                "query returned no results",
            )),
        }
    }

    pub async fn delete<M: Model>(&mut self, query: stmt::Select<M>) -> Result<()> {
        self.exec(query.delete()).await?;
        Ok(())
    }

    /// Execute a statement
    pub async fn exec<M: Model>(&mut self, statement: Statement<M>) -> Result<ValueStream> {
        let (tx, rx) = oneshot::channel();

        let conn = self.connection().await?;
        conn.in_tx
            .send(ConnOp::Exec {
                stmt: statement.untyped,
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    /// Execute a statement, assume only one record is returned
    #[doc(hidden)]
    pub async fn exec_one<M: Model>(&mut self, statement: Statement<M>) -> Result<stmt::Value> {
        let mut res = self.exec(statement).await?;
        let Some(ret) = res.next().await else {
            return Err(toasty_core::Error::record_not_found(
                "statement returned no results",
            ));
        };
        let next = res.next().await;
        let None = next else {
            return Err(toasty_core::Error::invalid_record_count(
                "expected 1 record, found multiple",
            ));
        };

        ret
    }

    /// Execute model creation
    ///
    /// Used by generated code
    #[doc(hidden)]
    pub async fn exec_insert_one<M: Model>(&mut self, mut stmt: stmt::Insert<M>) -> Result<M> {
        // TODO: HAX
        stmt.untyped.source.single = false;

        // Execute the statement and return the result
        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(self.shared.engine.schema.clone(), records);

        cursor.next().await.unwrap()
    }

    /// Creates tables and indices defined in the schema on the database.
    pub async fn push_schema(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let conn = self.connection().await?;
        conn.in_tx.send(ConnOp::PushSchema { tx }).unwrap();
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
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db")
            .field("engine", &self.shared.engine)
            .field("connected", &self.conn.is_some())
            .finish()
    }
}
