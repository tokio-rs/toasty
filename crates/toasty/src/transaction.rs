use std::sync::Arc;

use crate::{Executor, Result};

use toasty_core::{
    async_trait,
    driver::operation::{self, IsolationLevel},
    stmt::ValueStream,
    Schema,
};

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
    /// this holds the savepoint ID.
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
        db.exec_operation(
            operation::Transaction::Start {
                isolation,
                read_only,
            }
            .into(),
        )
        .await?;

        Ok(Transaction {
            db,
            finalized: false,
            savepoint: None,
        })
    }

    /// Commit the transaction.
    pub async fn commit(mut self) -> Result<()> {
        match self.savepoint {
            Some(savepoint) => self
                .db
                .exec_operation(operation::Transaction::ReleaseSavepoint(savepoint).into()),
            None => self
                .db
                .exec_operation(operation::Transaction::Commit.into()),
        }
        .await?;
        self.finalized = true;
        Ok(())
    }

    /// Roll back the transaction.
    pub async fn rollback(mut self) -> Result<()> {
        match self.savepoint {
            Some(savepoint) => self
                .db
                .exec_operation(operation::Transaction::RollbackToSavepoint(savepoint).into()),
            None => self
                .db
                .exec_operation(operation::Transaction::Rollback.into()),
        }
        .await?;
        self.finalized = true;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finalized {
            // Fire-and-forget rollback if the transaction is dropped without explicit
            // commit/rollback.
            let _ = match self.savepoint {
                Some(savepoint) => self
                    .db
                    .exec_operation(operation::Transaction::RollbackToSavepoint(savepoint).into()),
                None => self
                    .db
                    .exec_operation(operation::Transaction::Rollback.into()),
            };
        }
    }
}

#[async_trait]
impl<'a> Executor for Transaction<'a> {
    async fn transaction(&mut self) -> Result<Transaction<'_>> {
        let savepoint = 5;
        self.db
            .exec_operation(operation::Transaction::Savepoint(savepoint).into())
            .await?;

        Ok(Transaction {
            db: self.db,
            finalized: false,
            savepoint: Some(savepoint),
        })
    }

    async fn exec_untyped(&mut self, stmt: toasty_core::stmt::Statement) -> Result<ValueStream> {
        self.db.exec_untyped(stmt).await
    }

    fn schema(&mut self) -> &Arc<Schema> {
        self.db.schema()
    }
}
