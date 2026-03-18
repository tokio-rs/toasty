mod builder;
mod connect;
mod pool;

pub use builder::Builder;
pub use connect::*;
pub use pool::*;

use crate::{engine::Engine, Executor, Result, Transaction, TransactionBuilder};
pub(crate) use pool::{ConnectionHandle, ConnectionOperation};

use toasty_core::{async_trait, driver::Driver, stmt::ValueStream, Schema};

use std::sync::Arc;
use tokio::sync::oneshot;

/// Shared state between all `Db` clones.
pub(crate) struct Shared {
    pub(crate) engine: Engine,
    pub(crate) pool: Pool,
}

/// A database handle. Each instance owns (or will lazily acquire) a dedicated
/// connection from the pool. Cloning produces a new handle that will acquire its
/// own connection on first use. Dropping the [`Db`] instance will release the database connection
/// back to the pool.
pub struct Db {
    shared: Arc<Shared>,
    pub(crate) connection: Option<PoolConnection>,
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

    /// Lazily acquire a connection from the pool.
    pub(crate) async fn connection(&mut self) -> Result<&ConnectionHandle> {
        let conn = match &mut self.connection {
            Some(conn) => conn,
            empty => empty.insert(self.shared.pool.get().await?),
        };

        Ok(conn.handle())
    }

    pub(crate) async fn exec_operation(&mut self, operation: Operation) -> Result<Response> {
        let (tx, rx) = oneshot::channel();

        let conn = self.connection().await?;
        conn.in_tx
            .send(ConnectionOperation::ExecOperation {
                operation: Box::new(operation),
                tx,
            })
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
                in_transaction: false,
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    fn schema(&mut self) -> &Arc<Schema> {
        Db::schema(self)
    }

    fn capability(&mut self) -> &Capability {
        Db::capability(self)
    }
}
