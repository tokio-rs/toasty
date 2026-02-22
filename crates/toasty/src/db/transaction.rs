use std::{ops::Deref, sync::Arc, time::Duration};

use toasty_core::driver::{operation::Transaction as TransactionOp, transaction::IsolationLevel};
use tokio::sync::Mutex;

use crate::{db::ConnectionType, engine::Engine, Db};

pub(crate) struct Transaction {
    db: Option<Db>,
    done: bool,
}

impl Transaction {
    async fn exec_op(&self, op: TransactionOp) -> crate::Result<()> {
        let db = self.db.as_ref().unwrap();
        match &db.engine.connection {
            ConnectionType::Pool(_) => unreachable!(),
            ConnectionType::Transaction(arc) => {
                arc.lock()
                    .await
                    .exec(&db.engine.schema.db, op.into())
                    .await?;
            }
        }
        Ok(())
    }

    async fn commit_inner(&mut self) -> crate::Result<()> {
        // Mark done before the await: if this future is cancelled after the
        // query is dispatched (e.g. PostgreSQL sends it to a bg task on the
        // first poll), Drop must not issue a redundant ROLLBACK.
        self.done = true;
        self.exec_op(TransactionOp::Commit).await
    }

    async fn rollback_inner(&mut self) -> crate::Result<()> {
        // Same cancellation-safety reasoning as commit_inner.
        self.done = true;
        self.exec_op(TransactionOp::Rollback).await
    }
}

impl Deref for Transaction {
    type Target = Db;

    fn deref(&self) -> &Db {
        self.db.as_ref().unwrap()
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if let Some(db) = self.db.take() {
            // Extract the connection Arc synchronously — no reason to do this
            // inside the spawned task.
            if let ConnectionType::Transaction(arc) = &db.engine.connection {
                let arc = arc.clone();
                let schema = db.engine.schema.db.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = arc
                            .lock()
                            .await
                            .exec(&schema, TransactionOp::Rollback.into())
                            .await;
                    });
                }
            }
            // db (and its Arc<Mutex<PoolConnection>>) dropped here.
            // If no runtime was available the server cleans up on disconnect.
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
        let mut tx = self
            .db
            .begin_inner(self.isolation_level)
            .await
            .map_err(E::from)?;

        // The timeout covers only the user's callback, not BEGIN/COMMIT/ROLLBACK.
        let result = match self.timeout {
            Some(d) => match tokio::time::timeout(d, f(&*tx)).await {
                Ok(r) => r,
                Err(_elapsed) => Err(E::from(crate::Error::transaction_timed_out(d))),
            },
            None => f(&*tx).await,
        };

        match result {
            Ok(v) => match tx.commit_inner().await {
                Ok(()) => Ok(v),
                Err(e) if e.is_serialization_failure() || e.is_read_only_transaction() => {
                    let _ = tx.rollback_inner().await;
                    Err(E::from(e))
                }
                Err(e) => Err(E::from(e)),
            },
            Err(e) => {
                let _ = tx.rollback_inner().await;
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

    pub(crate) async fn begin_inner(
        &self,
        isolation: Option<IsolationLevel>,
    ) -> crate::Result<Transaction> {
        match &self.engine.connection {
            ConnectionType::Pool(pool) => {
                let conn = pool.get().await?;
                let db = Db {
                    driver: self.driver.clone(),
                    engine: Engine::new(
                        self.engine.schema.clone(),
                        ConnectionType::Transaction(Arc::new(Mutex::new(conn))),
                        self.engine.capabilities,
                    ),
                };
                let tx = Transaction {
                    db: Some(db),
                    done: false,
                };
                tx.exec_op(TransactionOp::Start { isolation }).await?;
                Ok(tx)
            }
            ConnectionType::Transaction(arc) => {
                // Nested: share the same connection (Arc clone). The driver
                // sees Transaction::Start on an in-progress connection and
                // creates a SAVEPOINT internally.
                let db = Db {
                    driver: self.driver.clone(),
                    engine: Engine::new(
                        self.engine.schema.clone(),
                        ConnectionType::Transaction(arc.clone()),
                        self.engine.capabilities,
                    ),
                };
                let tx = Transaction {
                    db: Some(db),
                    done: false,
                };
                // Isolation level only applies to the outermost transaction;
                // nested transactions use SAVEPOINTs, which ignore isolation.
                tx.exec_op(TransactionOp::Start { isolation: None }).await?;
                Ok(tx)
            }
        }
    }
}
