use std::{future::Future, ops::Deref, time::Duration};

use toasty_core::driver::{operation::Transaction as TransactionOp, transaction::IsolationLevel};
use tokio::sync::oneshot;

use crate::{
    db::{ConnectionType, EngineMsg},
    Db,
};

struct Transaction<'a> {
    done: bool,
    conn: TransactionConn<'a>,
}

enum TransactionConn<'a> {
    Root(Db),
    Nested(&'a Db),
}

impl Transaction<'_> {
    fn exec_op(&self, op: TransactionOp) -> impl Future<Output = crate::Result<()>> {
        let (tx, rx) = oneshot::channel();
        self.in_tx.send(EngineMsg::Transaction(op, tx)).unwrap();
        async { rx.await.unwrap() }
    }

    async fn commit(&mut self) -> crate::Result<()> {
        self.done = true;
        self.exec_op(TransactionOp::Commit).await
    }

    async fn rollback(&mut self) -> crate::Result<()> {
        self.done = true;
        self.exec_op(TransactionOp::Rollback).await
    }
}

impl Deref for Transaction<'_> {
    type Target = Db;

    fn deref(&self) -> &Db {
        match &self.conn {
            TransactionConn::Root(db) => db,
            TransactionConn::Nested(db) => db,
        }
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.done {
            // This synchronously sends a roleback command to the connection in the background task.
            // We don't need to await the future.
            std::mem::drop(self.exec_op(TransactionOp::Rollback));
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
            timeout: None,
            isolation_level: None,
        }
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
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

    pub async fn exec<O, E>(self, f: impl AsyncFnOnce(&Db) -> Result<O, E>) -> Result<O, E>
    where
        E: From<crate::Error>,
    {
        let mut tx = self.db.begin(self.isolation_level).await.map_err(E::from)?;

        // The timeout covers only the user's callback, not BEGIN/COMMIT/ROLLBACK.
        let result = match self.timeout {
            Some(d) => match tokio::time::timeout(d, f(&*tx)).await {
                Ok(r) => r,
                Err(_elapsed) => Err(E::from(crate::Error::transaction_timed_out(d))),
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
    pub async fn transaction<O, E>(&self, f: impl AsyncFnOnce(&Db) -> Result<O, E>) -> Result<O, E>
    where
        E: From<crate::Error>,
    {
        self.transaction_builder()
            .timeout(Duration::from_secs(5))
            .exec(f)
            .await
    }

    /// Return a builder for configuring a transaction.
    pub fn transaction_builder(&self) -> TransactionBuilder<'_> {
        TransactionBuilder::new(self)
    }

    async fn begin<'a>(
        &'a self,
        isolation: Option<IsolationLevel>,
    ) -> crate::Result<Transaction<'a>> {
        // We're wrapping the connection in a Transaction before the actual BEGIN is sent so if the
        // future is cancelled the wrapper rolls the transaction back.
        let tx = if self.in_transaction {
            Transaction {
                done: false,
                conn: TransactionConn::Nested(self),
            }
        } else {
            let conn = self.pool.get().await?;

            let db = Db::new(
                self.pool.clone(),
                self.schema.clone(),
                ConnectionType::Transaction(conn),
            );

            Transaction {
                done: false,
                conn: TransactionConn::Root(db),
            }
        };

        tx.exec_op(TransactionOp::Start { isolation }).await?;
        Ok(tx)
    }
}
