use super::Transaction;
use super::connection_task::{ConnectionHandle, ConnectionOperation};
use super::pool::Manager;
use super::tx::ConnRef;

use async_trait::async_trait;
use std::sync::Arc;
use toasty_core::{
    Schema,
    driver::{ExecResponse, operation::Operation},
    stmt,
};
use tokio::sync::oneshot;

/// A dedicated database connection retrieved from a pool.
///
/// Holding a `Connection` guarantees that all operations are executed on the
/// same physical connection. This is useful when multiple statements must
/// share connection-level state such as temporary tables or session variables.
///
/// When dropped, the connection is returned to the pool for reuse.
pub struct Connection {
    pub(super) inner: deadpool::managed::Object<Manager>,
    pub(super) shared: Arc<super::Shared>,
}

impl Connection {
    /// Access the underlying connection handle.
    pub(crate) fn handle(&self) -> &ConnectionHandle {
        &self.inner
    }

    /// Returns the compiled schema used by this connection.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.shared.engine.schema
    }

    pub(crate) async fn exec_stmt(
        &self,
        stmt: stmt::Statement,
        in_transaction: bool,
    ) -> crate::Result<ExecResponse> {
        let (tx, rx) = oneshot::channel();

        self.handle()
            .in_tx
            .send(ConnectionOperation::ExecStatement {
                stmt: Box::new(stmt),
                in_transaction,
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    pub(crate) async fn exec_operation(&self, operation: Operation) -> crate::Result<ExecResponse> {
        let (tx, rx) = oneshot::channel();

        self.handle()
            .in_tx
            .send(ConnectionOperation::ExecOperation {
                operation: Box::new(operation),
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    /// Begin a transaction on this connection.
    ///
    /// Takes `&mut self` so the `Connection` is exclusively borrowed while
    /// the transaction is open. This prevents statements from running on the
    /// connection directly — bypassing the transaction — when they should
    /// have gone through `&mut tx`.
    pub async fn transaction(&mut self) -> crate::Result<super::Transaction<'_>> {
        <Self as super::Executor>::transaction(self).await
    }

    /// Returns a [`TransactionBuilder`](super::TransactionBuilder) that will
    /// use this connection.
    ///
    /// Like [`transaction`](Self::transaction), this takes `&mut self` so the
    /// `Connection` stays locked for the lifetime of the transaction.
    pub fn transaction_builder(&mut self) -> super::TransactionBuilder<'_> {
        super::TransactionBuilder::new(super::tx::TxSource::Connection(self))
    }

    /// Creates tables and indices defined in the schema on the database.
    pub async fn push_schema(&self) -> crate::Result<()> {
        tracing::info!("pushing schema to database");
        let (tx, rx) = oneshot::channel();
        self.handle()
            .in_tx
            .send(ConnectionOperation::PushSchema { tx })
            .unwrap();
        rx.await.unwrap()
    }
}

#[async_trait]
impl super::Executor for Connection {
    async fn transaction(&mut self) -> crate::Result<Transaction<'_>> {
        Transaction::begin(ConnRef::Borrowed(self)).await
    }

    async fn exec_untyped(
        &mut self,
        stmt: toasty_core::stmt::Statement,
    ) -> crate::Result<ExecResponse> {
        self.exec_stmt(stmt, false).await
    }

    fn schema(&mut self) -> &Arc<Schema> {
        Connection::schema(self)
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("handle", &*self.inner)
            .finish()
    }
}
