use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::{db::ConnectionOperation, db::Executor, Result};

use async_trait::async_trait;
use toasty_core::{
    driver::operation::{self, IsolationLevel},
    stmt::Value,
    Schema,
};
use tokio::sync::oneshot;

/// Builder for configuring a transaction before starting it.
///
/// Collect isolation level and read-only settings, then call
/// [`begin`](Self::begin) with a [`Connection`](super::Connection) or
/// [`Db`](super::Db) to start the transaction.
pub struct TransactionBuilder {
    isolation: Option<IsolationLevel>,
    read_only: bool,
}

impl TransactionBuilder {
    /// Create a new builder with default settings (no explicit isolation
    /// level, read-write mode).
    pub fn new() -> Self {
        TransactionBuilder {
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

    /// Begin the transaction on the given connection.
    pub async fn begin(self, conn: &mut super::Connection) -> Result<Transaction<'_>> {
        Transaction::begin_with(ConnRef::Borrowed(conn), self.isolation, self.read_only).await
    }

    /// Begin the transaction on a freshly acquired connection from the pool.
    ///
    /// The connection is owned by the returned [`Transaction`] and will be
    /// returned to the pool when the transaction is dropped.
    pub async fn begin_on_db(self, db: &mut super::Db) -> Result<Transaction<'_>> {
        let conn = db.connection().await?;
        Transaction::begin_with(ConnRef::owned(conn), self.isolation, self.read_only).await
    }
}

impl Default for TransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// An active database transaction.
///
/// All operations executed through a `Transaction` are guaranteed to use the
/// same physical connection.
///
/// If dropped without calling [`commit`](Self::commit) or
/// [`rollback`](Self::rollback), the transaction is automatically rolled back.
pub struct Transaction<'a> {
    /// The connection this transaction operates on.
    conn: ConnRef<'a>,

    /// Whether commit or rollback has been called.
    finalized: bool,

    /// If this is a nested transaction (implemented through savepoints),
    /// this holds the savepoint stack depth to be used as an identifier.
    savepoint: Option<usize>,
}

/// Either a borrowed or owned reference to a [`Connection`](super::Connection).
pub(crate) enum ConnRef<'a> {
    Borrowed(&'a mut super::Connection),
    Owned(super::Connection, PhantomData<&'a ()>),
}

impl<'a> ConnRef<'a> {
    pub(crate) fn owned(conn: super::Connection) -> ConnRef<'a> {
        ConnRef::Owned(conn, PhantomData)
    }
}

impl Deref for ConnRef<'_> {
    type Target = super::Connection;

    fn deref(&self) -> &Self::Target {
        match self {
            ConnRef::Borrowed(c) => c,
            ConnRef::Owned(c, _) => c,
        }
    }
}

impl DerefMut for ConnRef<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            ConnRef::Borrowed(c) => c,
            ConnRef::Owned(c, _) => c,
        }
    }
}

impl<'a> Transaction<'a> {
    pub(crate) async fn begin(conn: ConnRef<'a>) -> Result<Transaction<'a>> {
        Self::begin_with(conn, None, false).await
    }

    pub(crate) async fn begin_with(
        conn: ConnRef<'a>,
        isolation: Option<IsolationLevel>,
        read_only: bool,
    ) -> Result<Transaction<'a>> {
        tracing::debug!(
            isolation = ?isolation,
            read_only = read_only,
            "beginning transaction"
        );

        // We're creating the Transaction struct before actually starting the transaction. If the
        // future is cancelled while waiting on the response of the start command, the transaction
        // is still rolled back.
        let tx = Transaction {
            conn,
            finalized: false,
            savepoint: None,
        };

        tx.conn
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
        tracing::debug!("committing transaction");
        // Because driver operations are done in a background task, all the operations aren't
        // cancelled and will continue even if this future is dropped. Setting the finalized flag
        // to true early here makes sure that if the future is dropped we don't queue a rollback
        // command.
        self.finalized = true;
        match self.savepoint {
            Some(_) => self
                .conn
                .exec_operation(operation::Transaction::ReleaseSavepoint(self.savepoint()).into()),
            None => self
                .conn
                .exec_operation(operation::Transaction::Commit.into()),
        }
        .await?;
        Ok(())
    }

    /// Roll back the transaction.
    pub async fn rollback(mut self) -> Result<()> {
        tracing::debug!("rolling back transaction");
        // See `commit` why we're setting the finalized flag to true early.
        self.finalized = true;
        match self.savepoint {
            Some(_) => self.conn.exec_operation(
                operation::Transaction::RollbackToSavepoint(self.savepoint()).into(),
            ),
            None => self
                .conn
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
            // connection task without awaiting the response.
            let (tx, _rx) = oneshot::channel();
            let _ = self
                .conn
                .handle()
                .in_tx
                .send(ConnectionOperation::ExecOperation {
                    operation: Box::new(op.into()),
                    tx,
                });
        }
    }
}

#[async_trait]
impl<'a> Executor for Transaction<'a> {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let depth = match self.savepoint {
            Some(savepoint) => savepoint + 1,
            None => 1,
        };
        tracing::debug!(depth = depth, "creating nested transaction (savepoint)");

        let transaction = Transaction {
            conn: ConnRef::Borrowed(&mut self.conn),
            finalized: false,
            savepoint: Some(depth),
        };

        transaction
            .conn
            .exec_operation(operation::Transaction::Savepoint(transaction.savepoint()).into())
            .await?;

        Ok(transaction)
    }

    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<Value> {
        self.conn.exec_stmt(stmt, true).await
    }

    async fn exec_paginated(
        &mut self,
        stmt: toasty_core::stmt::Statement,
    ) -> Result<crate::engine::exec::ExecResponse> {
        self.conn.exec_paginated(stmt).await
    }

    fn schema(&mut self) -> &Arc<Schema> {
        self.conn.schema()
    }
}
