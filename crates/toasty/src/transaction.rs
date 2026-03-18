use std::sync::Arc;

use crate::{db::ConnectionOperation, Executor, Result};

use toasty_core::{
    async_trait,
    driver::{
        operation::{self, IsolationLevel},
        Capability,
    },
    stmt::ValueStream,
    Schema,
};
use tokio::sync::oneshot;

/// Builder for configuring a transaction before starting it.
pub struct TransactionBuilder<'db> {
    db: &'db mut crate::Db,
    isolation: Option<IsolationLevel>,
    read_only: bool,
}

impl<'db> TransactionBuilder<'db> {
    pub(crate) fn new(db: &'db mut crate::Db) -> Self {
        TransactionBuilder {
            db,
            isolation: None,
            read_only: false,
        }
    }

    /// Set the isolation level for this transaction.
    pub fn isolation(mut self, level: IsolationLevel) -> Self {
        self.isolation = Some(level);
        self
    }

    /// Set whether this transaction is read-only.
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Begin the transaction with the configured options.
    pub async fn begin(self) -> Result<Transaction<'db>> {
        Transaction::begin_with(self.db, self.isolation, self.read_only).await
    }
}

/// An active database transaction.
///
/// Borrows `&mut Db` for its lifetime, preventing concurrent use of the
/// same Db handle while a transaction is open.
///
/// If dropped without calling [`commit`](Self::commit) or
/// [`rollback`](Self::rollback), the transaction is automatically rolled back.
pub struct Transaction<'db> {
    /// Holds the mutable borrow of Db to prevent concurrent use.
    db: &'db mut crate::Db,

    /// Cloned engine for schema access and query compilation.
    /// Whether commit or rollback has been called.
    finalized: bool,

    /// If this is a nested transaction (implemented through savepoints),
    /// this holds the savepoint stack depth to be used as an identifier.
    savepoint: Option<usize>,
}

impl<'db> Transaction<'db> {
    pub(crate) async fn begin(db: &'db mut crate::Db) -> Result<Transaction<'db>> {
        Self::begin_with(db, None, false).await
    }

    pub(crate) async fn begin_with(
        db: &'db mut crate::Db,
        isolation: Option<IsolationLevel>,
        read_only: bool,
    ) -> Result<Transaction<'db>> {
        // We're creating the Transaction struct before actually starting the transaction. If the
        // future is cancelled while waiting on the response of the start command, the transaction
        // is still rolled back.
        let tx = Transaction {
            db,
            finalized: false,
            savepoint: None,
        };

        tx.db
            .exec_operation(
                operation::Transaction::Start {
                    isolation,
                    read_only,
                }
                .into(),
            )
            .await?;
        Ok(tx)
    }

    /// Commit the transaction.
    pub async fn commit(mut self) -> Result<()> {
        // Because driver operations are done in a background task, all the operations aren't
        // cancelled and will continue even if this future is dropped. Setting the finalized flag
        // to true early here makes sure that if the future is dropped we don't queue a rollback
        // command.
        self.finalized = true;
        match self.savepoint {
            Some(_) => self
                .db
                .exec_operation(operation::Transaction::ReleaseSavepoint(self.savepoint()).into()),
            None => self
                .db
                .exec_operation(operation::Transaction::Commit.into()),
        }
        .await?;
        Ok(())
    }

    /// Roll back the transaction.
    pub async fn rollback(mut self) -> Result<()> {
        // See `commit` why we're setting the finalized flag to true early.
        self.finalized = true;
        match self.savepoint {
            Some(_) => self.db.exec_operation(
                operation::Transaction::RollbackToSavepoint(self.savepoint()).into(),
            ),
            None => self
                .db
                .exec_operation(operation::Transaction::Rollback.into()),
        }
        .await?;
        Ok(())
    }

    fn savepoint(&self) -> String {
        format!("tx_{}", self.savepoint.unwrap())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finalized {
            let op = match self.savepoint {
                Some(_) => operation::Transaction::RollbackToSavepoint(self.savepoint()),
                None => operation::Transaction::Rollback,
            };

            // Fire-and-forget rollback: send the operation to the background
            // connection task without awaiting the response. By the time a
            // transaction exists, `begin` already acquired the connection, so
            // it is always cached.
            if let Some(conn) = self.db.connection.as_ref() {
                let (tx, _rx) = oneshot::channel();
                let _ = conn
                    .handle()
                    .in_tx
                    .send(ConnectionOperation::ExecOperation {
                        operation: Box::new(op.into()),
                        tx,
                    });
            }
        }
    }
}

#[async_trait]
impl<'a> Executor for Transaction<'a> {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let transaction = Transaction {
            db: self.db,
            finalized: false,
            savepoint: Some(match self.savepoint {
                Some(savepoint) => savepoint + 1,
                None => 1,
            }),
        };

        transaction
            .db
            .exec_operation(operation::Transaction::Savepoint(transaction.savepoint()).into())
            .await?;

        Ok(transaction)
    }

    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<ValueStream> {
        let (tx, rx) = oneshot::channel();

        let conn = self.db.connection().await?;
        conn.in_tx
            .send(ConnectionOperation::ExecStatement {
                stmt: Box::new(stmt),
                in_transaction: true,
                tx,
            })
            .unwrap();

        rx.await.unwrap()
    }

    fn schema(&mut self) -> &Arc<Schema> {
        self.db.schema()
    }

    fn capability(&mut self) -> &Capability {
        self.db.capability()
    }
}
