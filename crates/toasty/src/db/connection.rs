use super::pool::{ConnectionHandle, ConnectionOperation, Manager};
use super::tx::ConnRef;
use super::Transaction;

use async_trait::async_trait;
use std::sync::Arc;
use toasty_core::{
    driver::{operation::Operation, Response},
    stmt::{self, Value},
    Schema,
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
    ) -> crate::Result<Value> {
        let returns_list = match &stmt {
            stmt::Statement::Query(q) => !q.single,
            stmt::Statement::Insert(i) => !i.source.single,
            stmt::Statement::Update(i) => match &i.target {
                stmt::UpdateTarget::Query(q) => !q.single,
                stmt::UpdateTarget::Model(_) => false,
                _ => true,
            },
            stmt::Statement::Delete(d) => !d.selection().single,
        };

        let (tx, rx) = oneshot::channel();

        self.handle()
            .in_tx
            .send(ConnectionOperation::ExecStatement {
                stmt: Box::new(stmt),
                in_transaction,
                tx,
            })
            .unwrap();

        let mut stream = rx.await.unwrap()?;

        if returns_list {
            let values = stream.collect().await?;
            Ok(Value::List(values))
        } else {
            match stream.next().await {
                Some(value) => value,
                None => Ok(Value::Null),
            }
        }
    }

    pub(crate) async fn exec_operation(&self, operation: Operation) -> crate::Result<Response> {
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

impl Connection {
    /// Create a [`super::TransactionBuilder`] for configuring transaction
    /// options (isolation level, read-only) before starting it.
    pub fn transaction_builder(&self) -> super::TransactionBuilder {
        super::TransactionBuilder::new()
    }
}

#[async_trait]
impl super::Executor for Connection {
    async fn transaction(&mut self) -> crate::Result<Transaction<'_>> {
        Transaction::begin(ConnRef::Borrowed(self)).await
    }

    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> crate::Result<Value> {
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
