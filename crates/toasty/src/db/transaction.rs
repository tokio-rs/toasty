use std::{future::Future, ops::Deref, sync::atomic::Ordering, time::Duration};

use toasty_core::driver::{operation::Transaction as TransactionOp, transaction::IsolationLevel};
use tokio::sync::oneshot;

use crate::{
    db::{ConnectionSource, EngineMsg},
    Db,
};

struct Transaction<'a> {
    done: bool,
    conn: TransactionConn<'a>,
}

enum TransactionConn<'a> {
    Root(Db),
    Nested { db: &'a Db, depth: usize },
}

impl Transaction<'_> {
    fn exec_op(&self, op: TransactionOp) -> impl Future<Output = crate::Result<()>> {
        let (tx, rx) = oneshot::channel();
        self.in_tx.send(EngineMsg::Transaction(op, tx)).unwrap();
        async { rx.await.unwrap() }
    }

    async fn commit(&mut self) -> crate::Result<()> {
        // We're marking the transaction as done before doing the actual operation since the actual
        // work is being done in a bg task. Even if the Transaction is dropped after the first poll
        // the operation is finished in the bg task and should not be rolled back on drop.
        self.done = true;
        let op = match &self.conn {
            TransactionConn::Root(_) => TransactionOp::Commit,
            TransactionConn::Nested { depth, .. } => TransactionOp::ReleaseSavepoint(*depth),
        };
        self.exec_op(op).await
    }

    async fn rollback(&mut self) -> crate::Result<()> {
        // See commit why we're marking this transaction as done early here.
        self.done = true;
        self.exec_op(self.rollback_op()).await
    }

    fn rollback_op(&self) -> TransactionOp {
        match &self.conn {
            TransactionConn::Root(_) => TransactionOp::Rollback,
            TransactionConn::Nested { depth, .. } => TransactionOp::RollbackToSavepoint(*depth),
        }
    }
}

impl Deref for Transaction<'_> {
    type Target = Db;

    fn deref(&self) -> &Db {
        match &self.conn {
            TransactionConn::Root(db) => db,
            TransactionConn::Nested { db, .. } => db,
        }
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.done {
            std::mem::drop(self.exec_op(self.rollback_op()));
        }
    }
}

pub struct TransactionBuilder<'a> {
    db: &'a Db,
    timeout: Option<Duration>,
    isolation_level: Option<IsolationLevel>,
}

impl<'a> TransactionBuilder<'a> {
    pub(crate) fn new(db: &'a Db) -> Self {
        Self {
            db,
            timeout: Some(Duration::from_secs(5)),
            isolation_level: None,
        }
    }

    /// Sets the transaction timeout. Defaults to 5 seconds. Pass `None` to disable.
    pub fn timeout(mut self, duration: impl Into<Option<Duration>>) -> Self {
        self.timeout = duration.into();
        self
    }

    /// Allows dirty reads. Not supported by all drivers.
    pub fn read_uncommitted(mut self) -> Self {
        self.isolation_level = Some(IsolationLevel::ReadUncommitted);
        self
    }

    /// Prevents dirty reads; non-repeatable reads and phantom reads are still possible.
    pub fn read_committed(mut self) -> Self {
        self.isolation_level = Some(IsolationLevel::ReadCommitted);
        self
    }

    /// Prevents dirty and non-repeatable reads; phantom reads are still possible.
    pub fn repeatable_read(mut self) -> Self {
        self.isolation_level = Some(IsolationLevel::RepeatableRead);
        self
    }

    /// Full isolation. May produce serialization failures that require retry.
    pub fn serializable(mut self) -> Self {
        self.isolation_level = Some(IsolationLevel::Serializable);
        self
    }

    /// Transactions are only supported by SQL drivers (SQLite, PostgreSQL, MySQL).
    /// Prefer using batch operations when possible. Use transactions only when a
    /// batch operation cannot express the required atomicity.
    pub async fn exec<O, E>(self, f: impl AsyncFnOnce(&Db) -> Result<O, E>) -> Result<O, E>
    where
        E: From<crate::Error>,
    {
        let mut tx = self.db.begin(self.isolation_level).await.map_err(E::from)?;

        let result = match self.timeout {
            Some(d) => match tokio::time::timeout(d, f(&*tx)).await {
                Ok(r) => r,
                Err(_) => Err(E::from(crate::Error::transaction_timeout(d))),
            },
            None => f(&*tx).await,
        };

        match result {
            Ok(v) => match tx.commit().await {
                Ok(()) => Ok(v),
                Err(e) if e.is_serialization_failure() || e.is_read_only_transaction() => {
                    let _ = tx.rollback().await;
                    Err(E::from(e))
                }
                Err(e) => Err(E::from(e)),
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

impl Db {
    /// Execute a transaction with a 5-second default timeout.
    ///
    /// Transactions are only supported by SQL drivers (SQLite, PostgreSQL, MySQL).
    /// Prefer using batch operations when possible. Use transactions only when a
    /// batch operation cannot express the required atomicity.
    pub async fn transaction<O, E>(&self, f: impl AsyncFnOnce(&Db) -> Result<O, E>) -> Result<O, E>
    where
        E: From<crate::Error>,
    {
        self.transaction_builder().exec(f).await
    }

    /// Return a builder for configuring a transaction.
    ///
    /// Transactions are only supported by SQL drivers. Prefer using batch operations when
    /// possible.
    pub fn transaction_builder(&self) -> TransactionBuilder<'_> {
        TransactionBuilder::new(self)
    }

    async fn begin<'a>(
        &'a self,
        isolation: Option<IsolationLevel>,
    ) -> crate::Result<Transaction<'a>> {
        // We're wrapping the connection in a Transaction before the actual BEGIN is sent so if the
        // future is cancelled the wrapper rolls the transaction back.
        let (tx, start_op) = match &self.savepoint_depth {
            Some(counter) => {
                // Nested: increment depth and create a savepoint
                let depth = counter.fetch_add(1, Ordering::Relaxed);

                let tx = Transaction {
                    done: false,
                    conn: TransactionConn::Nested { db: self, depth },
                };

                (tx, TransactionOp::Savepoint(depth))
            }
            None => {
                // Root: acquire a connection and start a transaction
                let conn = self.pool.get().await?;

                let db = Db::new(
                    self.pool.clone(),
                    self.schema.clone(),
                    ConnectionSource::Transaction(conn),
                );

                let tx = Transaction {
                    done: false,
                    conn: TransactionConn::Root(db),
                };

                (tx, TransactionOp::Start { isolation })
            }
        };

        tx.exec_op(start_op).await?;
        Ok(tx)
    }
}
