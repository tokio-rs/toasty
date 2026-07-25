use super::Transaction;
use super::connection_task::{ConnectionHandle, ConnectionOperation};
use super::pool::Manager;
use super::tx::ConnRef;

use async_trait::async_trait;
use std::sync::Arc;
use toasty_core::{
    Schema,
    driver::{
        Capability, ExecResponse,
        operation::{Operation, RawSql},
    },
    stmt,
};
use tokio::sync::oneshot;
use tracing::Instrument;

/// A dedicated connection acquired from a database handle.
///
/// Holding a `Connection` guarantees that all operations are executed on the
/// same physical connection. This is useful when multiple statements must
/// share connection-level state such as temporary tables or session variables.
///
/// When dropped, the connection returns to its pool or releases a direct
/// connection for the next caller.
pub struct Connection {
    pub(super) inner: ConnectionInner,
    pub(super) shared: Arc<super::Shared>,
}

pub(super) enum ConnectionInner {
    Pooled(deadpool::managed::Object<Manager>),
    Direct(tokio::sync::Mutex<tokio::sync::OwnedMutexGuard<Box<dyn toasty_core::Connection>>>),
}

impl Connection {
    /// Access the underlying connection handle.
    pub(crate) fn handle(&self) -> Option<&ConnectionHandle> {
        match &self.inner {
            ConnectionInner::Pooled(inner) => Some(inner),
            ConnectionInner::Direct(_) => None,
        }
    }

    /// Returns the compiled schema used by this connection.
    pub fn schema(&self) -> &Arc<Schema> {
        &self.shared.engine.schema
    }

    pub(crate) async fn exec_stmt(
        &mut self,
        stmt: stmt::Statement,
        in_transaction: bool,
    ) -> crate::Result<ExecResponse> {
        // Created on the caller's task so the span parents to the caller's
        // current span; the worker task enters it while executing.
        let span = crate::instrument::query_span(&self.shared.engine.schema, &stmt);

        match &mut self.inner {
            ConnectionInner::Pooled(inner) => {
                let (tx, rx) = oneshot::channel();
                inner
                    .in_tx
                    .send(ConnectionOperation::ExecStatement {
                        stmt: Box::new(stmt),
                        in_transaction,
                        span: span.clone(),
                        tx,
                    })
                    .unwrap();
                rx.instrument(span).await.unwrap()
            }
            ConnectionInner::Direct(inner) => {
                self.shared
                    .engine
                    .exec_buffered(&mut ***inner.get_mut(), stmt, in_transaction)
                    .instrument(span)
                    .await
            }
        }
    }

    pub(crate) async fn exec_operation(
        &mut self,
        operation: Operation,
    ) -> crate::Result<ExecResponse> {
        match &mut self.inner {
            ConnectionInner::Pooled(inner) => {
                let (tx, rx) = oneshot::channel();
                inner
                    .in_tx
                    .send(ConnectionOperation::ExecOperation {
                        operation: Box::new(operation),
                        span: tracing::Span::current(),
                        tx,
                    })
                    .unwrap();
                rx.await.unwrap()
            }
            ConnectionInner::Direct(inner) => {
                inner
                    .get_mut()
                    .exec(&self.shared.engine.schema, operation)
                    .await
            }
        }
    }

    pub(crate) async fn exec_raw_sql(&mut self, raw: RawSql) -> crate::Result<ExecResponse> {
        let span = crate::instrument::raw_sql_span();
        match &mut self.inner {
            ConnectionInner::Pooled(inner) => {
                let (tx, rx) = oneshot::channel();
                inner
                    .in_tx
                    .send(ConnectionOperation::ExecRawSql {
                        raw: Box::new(raw),
                        span: span.clone(),
                        tx,
                    })
                    .unwrap();
                rx.instrument(span).await.unwrap()
            }
            ConnectionInner::Direct(inner) => {
                self.shared
                    .engine
                    .exec_raw_sql(&mut ***inner.get_mut(), raw)
                    .instrument(span)
                    .await
            }
        }
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
    pub async fn push_schema(&mut self) -> crate::Result<()> {
        tracing::info!("pushing schema to database");
        match &mut self.inner {
            ConnectionInner::Pooled(inner) => {
                let (tx, rx) = oneshot::channel();
                inner
                    .in_tx
                    .send(ConnectionOperation::PushSchema {
                        span: tracing::Span::current(),
                        tx,
                    })
                    .unwrap();
                rx.await.unwrap()
            }
            ConnectionInner::Direct(inner) => {
                inner
                    .get_mut()
                    .push_schema(&self.shared.engine.schema)
                    .await
            }
        }
    }

    /// Checks whether this connection can reach its database.
    pub async fn ping(&mut self) -> crate::Result<()> {
        match &mut self.inner {
            ConnectionInner::Pooled(inner) => {
                let (tx, rx) = oneshot::channel();
                inner
                    .in_tx
                    .send(ConnectionOperation::Ping {
                        span: tracing::Span::current(),
                        tx,
                    })
                    .unwrap();
                rx.await.unwrap()
            }
            ConnectionInner::Direct(inner) => inner.get_mut().ping().await,
        }
    }
}

#[async_trait]
impl super::Executor for Connection {
    async fn transaction(&mut self) -> crate::Result<Transaction<'_>> {
        super::require_interactive_transactions(self.shared.engine.capability())?;
        Transaction::begin(ConnRef::Borrowed(self)).await
    }

    async fn exec_untyped(
        &mut self,
        stmt: toasty_core::stmt::Statement,
    ) -> crate::Result<ExecResponse> {
        self.exec_stmt(stmt, false).await
    }

    async fn exec_raw_sql(&mut self, raw: RawSql) -> crate::Result<ExecResponse> {
        Connection::exec_raw_sql(self, raw).await
    }

    fn capability(&mut self) -> &Capability {
        self.shared.engine.capability()
    }

    fn schema(&mut self) -> &Arc<Schema> {
        Connection::schema(self)
    }
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let strategy = match &self.inner {
            ConnectionInner::Pooled(_) => "pooled",
            ConnectionInner::Direct(_) => "direct",
        };
        f.debug_struct("Connection")
            .field("strategy", &strategy)
            .finish_non_exhaustive()
    }
}
