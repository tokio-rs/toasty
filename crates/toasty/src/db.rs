mod builder;
mod connect;
mod connection;
mod executor;
mod pool;
mod tx;

pub use builder::Builder;
pub use connect::Connect;
pub use connection::Connection;
pub use executor::Executor;
pub use pool::{Pool, PoolConfig, PoolStatus, Timeouts};
pub use toasty_core::driver::{Capability, Driver};
pub use tx::{Transaction, TransactionBuilder};

pub(crate) use pool::ConnectionOperation;
pub(crate) use tx::ConnRef;

use crate::{Result, engine::Engine};

use async_trait::async_trait;
use toasty_core::{
    Schema,
    stmt::{self, Value},
};

use std::sync::Arc;

/// Shared state between all `Db` clones.
pub(crate) struct Shared {
    pub(crate) engine: Engine,
    pub(crate) pool: Pool,
}

/// A database handle backed by a connection pool.
///
/// Each operation acquires a connection from the pool, executes, and returns
/// the connection. Use [`Db::connection`] to obtain a dedicated
/// [`Connection`] when you need multiple statements to share the same
/// physical connection (e.g. temporary tables or session-level state).
///
/// Cloning a `Db` is cheap — it shares the underlying pool.
pub struct Db {
    shared: Arc<Shared>,
}

impl Clone for Db {
    fn clone(&self) -> Self {
        Db {
            shared: self.shared.clone(),
        }
    }
}

impl Db {
    /// Create a new [`Builder`] for configuring and opening a database.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// let db = toasty::Db::builder()
    ///     .register::<User>()
    ///     .build(driver)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Acquire a dedicated connection from the pool.
    ///
    /// The returned [`Connection`] implements [`Executor`] and pins all
    /// operations to the same physical connection. This is useful when
    /// multiple statements must share connection-level state such as
    /// temporary tables or session variables.
    ///
    /// When the `Connection` is dropped it is returned to the pool for reuse.
    pub async fn connection(&self) -> Result<Connection> {
        self.shared.pool.get(self.shared.clone()).await
    }

    pub(crate) async fn exec_stmt(
        &self,
        stmt: stmt::Statement,
        in_transaction: bool,
    ) -> Result<Value> {
        let conn = self.connection().await?;
        conn.exec_stmt(stmt, in_transaction).await
    }

    /// Create a [`TransactionBuilder`] for configuring transaction options
    /// (isolation level, read-only) before starting it.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// # let mut db = toasty::Db::builder().register::<User>().build(driver).await.unwrap();
    /// let mut conn = db.connection().await.unwrap();
    /// let tx = toasty::TransactionBuilder::new()
    ///     .read_only(true)
    ///     .begin(&mut conn)
    ///     .await
    ///     .unwrap();
    /// tx.commit().await.unwrap();
    /// # });
    /// ```
    pub fn transaction_builder(&self) -> TransactionBuilder {
        TransactionBuilder::new()
    }

    /// Creates tables and indices defined in the schema on the database.
    pub async fn push_schema(&self) -> Result<()> {
        let conn = self.connection().await?;
        conn.push_schema().await
    }

    /// Drops the entire database and recreates an empty one without applying migrations.
    pub async fn reset_db(&self) -> Result<()> {
        self.shared.pool.driver().reset_db().await
    }

    /// Returns a reference to the underlying database driver.
    pub fn driver(&self) -> &dyn Driver {
        self.shared.pool.driver()
    }

    /// Returns the compiled schema used by this database handle.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.shared.engine.schema
    }

    /// Returns the capability flags reported by the driver.
    ///
    /// The query engine uses these to decide which operation types to generate
    /// (e.g., SQL vs. key-value).
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
            .finish()
    }
}

#[async_trait]
impl Executor for Db {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let conn = self.connection().await?;
        Transaction::begin(ConnRef::owned(conn)).await
    }

    async fn exec_untyped(&mut self, stmt: stmt::Statement) -> Result<Value> {
        self.exec_stmt(stmt, false).await
    }

    fn schema(&mut self) -> &Arc<Schema> {
        Db::schema(self)
    }
}
