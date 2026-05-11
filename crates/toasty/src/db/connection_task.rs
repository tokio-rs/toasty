//! Per-connection worker task and its message protocol.
//!
//! Each pooled connection is owned by a [`ConnectionTask`] running in a
//! dedicated tokio task; callers interact with it through a
//! [`ConnectionHandle`] by sending [`ConnectionOperation`] messages over
//! an mpsc channel and awaiting a oneshot response. This serializes
//! access to the underlying driver connection — drivers are not
//! required to be `Sync` on `exec` — and gives the connection its own
//! lifecycle independent of the pool object that holds the handle.

use std::sync::Arc;

use toasty_core::driver::{Connection, Rows};
use toasty_core::stmt::Value;
use tokio::{
    sync::{Notify, mpsc, oneshot},
    task::JoinHandle,
};

use crate::engine::Engine;

/// Operations sent to the connection task.
pub(crate) enum ConnectionOperation {
    /// Execute a statement (compile + run on the connection).
    ExecStatement {
        stmt: Box<toasty_core::stmt::Statement>,
        in_transaction: bool,
        tx: oneshot::Sender<crate::Result<toasty_core::driver::ExecResponse>>,
    },
    ExecOperation {
        operation: Box<toasty_core::driver::operation::Operation>,
        tx: oneshot::Sender<crate::Result<toasty_core::driver::ExecResponse>>,
    },
    /// Push schema to the database.
    PushSchema {
        tx: oneshot::Sender<crate::Result<()>>,
    },
    /// Active liveness probe issued by the pool's background sweep
    /// (and, eventually, per-acquire pre-ping). Routes through the
    /// connection task so the ping is serialized against any in-flight
    /// `exec` on the same connection.
    Ping {
        tx: oneshot::Sender<crate::Result<()>>,
    },
}

/// Handle to a dedicated connection task.
///
/// When dropped, `in_tx` closes the channel, causing the background task to
/// finish processing remaining messages and exit gracefully.
pub(crate) struct ConnectionHandle {
    pub(crate) in_tx: mpsc::UnboundedSender<ConnectionOperation>,
    join_handle: JoinHandle<()>,
}

impl ConnectionHandle {
    /// Spawn a worker task that owns `connection` and serializes
    /// operations against it. `sweep_notify` is signalled whenever the
    /// task observes that the connection went bad so the pool's
    /// health-check sweep can escalate immediately rather than waiting
    /// for its next periodic tick.
    pub(crate) fn spawn(
        connection: Box<dyn Connection>,
        engine: Engine,
        sweep_notify: Arc<Notify>,
    ) -> Self {
        let (in_tx, in_rx) = mpsc::unbounded_channel::<ConnectionOperation>();
        let task = ConnectionTask {
            connection,
            engine,
            in_rx,
            sweep_notify,
        };
        let join_handle = tokio::spawn(task.run());
        Self { in_tx, join_handle }
    }

    /// Returns true once the worker task has exited. Used by the pool's
    /// `recycle` to detect dead slots.
    pub(crate) fn is_finished(&self) -> bool {
        self.join_handle.is_finished()
    }
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("channel_closed", &self.in_tx.is_closed())
            .field("task_finished", &self.join_handle.is_finished())
            .finish()
    }
}

/// Owns one database connection and serializes operations against it.
/// Spawned by [`ConnectionHandle::spawn`]; runs until `in_rx` closes
/// (sender side dropped) or the connection reports `is_valid() == false`.
struct ConnectionTask {
    connection: Box<dyn Connection>,
    engine: Engine,
    in_rx: mpsc::UnboundedReceiver<ConnectionOperation>,
    sweep_notify: Arc<Notify>,
}

impl ConnectionTask {
    async fn run(mut self) {
        while let Some(op) = self.in_rx.recv().await {
            if !self.handle(op).await {
                return;
            }
        }
    }

    /// Dispatch one operation. Returns `false` if the connection went bad
    /// during it and the task should exit.
    async fn handle(&mut self, op: ConnectionOperation) -> bool {
        match op {
            ConnectionOperation::ExecStatement {
                stmt,
                in_transaction,
                tx,
            } => {
                let result = self.exec_statement(*stmt, in_transaction).await;
                self.respond(tx, result)
            }
            ConnectionOperation::ExecOperation { operation, tx } => {
                let result = self.connection.exec(&self.engine.schema, *operation).await;
                self.respond(tx, result)
            }
            ConnectionOperation::PushSchema { tx } => {
                let result = self.connection.push_schema(&self.engine.schema).await;
                self.respond(tx, result)
            }
            ConnectionOperation::Ping { tx } => {
                let result = self.connection.ping().await;
                self.respond(tx, result)
            }
        }
    }

    async fn exec_statement(
        &mut self,
        stmt: toasty_core::stmt::Statement,
        in_transaction: bool,
    ) -> crate::Result<toasty_core::driver::ExecResponse> {
        let single = stmt.is_single();
        let mut response = self
            .engine
            .exec(&mut *self.connection, stmt, in_transaction)
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

    /// Send `result` to the caller, but if the connection reported
    /// invalid during the op, close `in_rx` *first* so the mpsc transitions
    /// to closed synchronously. A consumer woken by the response that then
    /// re-enters the pool sees `in_tx.is_closed() == true` in
    /// `Manager::recycle` and the slot is evicted — no race with this
    /// task's exit. Returns whether the task should keep running.
    ///
    /// Also wakes the pool's background sweep so it can escalate to a
    /// full idle-pool ping pass without waiting for the next periodic
    /// tick. A real connection-lost error usually means more than one
    /// connection in the pool is affected (database restart, network
    /// event), and catching the rest eagerly turns a multi-second
    /// recovery window into one round-trip per idle slot.
    fn respond<T>(&mut self, tx: oneshot::Sender<T>, result: T) -> bool {
        if self.connection.is_valid() {
            let _ = tx.send(result);
            true
        } else {
            tracing::debug!("connection reported invalid; closing channel and exiting");
            self.in_rx.close();
            self.sweep_notify.notify_one();
            let _ = tx.send(result);
            false
        }
    }
}
