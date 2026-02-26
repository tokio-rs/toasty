use crate::{db::PoolConnection, engine::Engine, Executor, Result};

use toasty_core::driver::operation::{self, IsolationLevel};

/// An active database transaction.
///
/// Borrows `&mut Db` for its lifetime, preventing concurrent use of the
/// same Db handle while a transaction is open.
///
/// If dropped without calling [`commit`](Self::commit) or
/// [`rollback`](Self::rollback), the transaction is automatically rolled back.
pub struct Transaction<'db> {
    /// Holds the mutable borrow of Db to prevent concurrent use.
    _db: &'db mut crate::Db,

    /// Cloned engine for schema access and query compilation.
    engine: Engine,

    /// Pinned connection for the duration of the transaction.
    /// `Option` so that `Drop` can `.take()` and move it into a spawned task.
    connection: Option<PoolConnection>,

    /// Whether commit or rollback has been called.
    committed: bool,

    /// Monotonic counter for generating unique savepoint names.
    savepoint_counter: usize,

    /// When a savepoint is dropped it cannot make an asynchronous request.
    /// Instead, it stores its ID here so we can roll back later.
    pending_savepoint_rollback: Option<usize>,
}

impl<'db> Transaction<'db> {
    pub(crate) async fn begin(db: &'db mut crate::Db) -> Result<Transaction<'db>> {
        let engine = db.engine.clone();
        let mut connection = engine.pool.get().await?;

        connection
            .exec(&engine.schema.db, operation::Transaction::start().into())
            .await?;

        Ok(Transaction {
            _db: db,
            engine,
            connection: Some(connection),
            committed: false,
            savepoint_counter: 0,
            pending_savepoint_rollback: None,
        })
    }

    pub(crate) async fn begin_with(
        db: &'db mut crate::Db,
        isolation: Option<IsolationLevel>,
        read_only: bool,
    ) -> Result<Transaction<'db>> {
        let engine = db.engine.clone();
        let mut connection = engine.pool.get().await?;

        connection
            .exec(
                &engine.schema.db,
                operation::Transaction::Start {
                    isolation,
                    read_only,
                }
                .into(),
            )
            .await?;

        Ok(Transaction {
            _db: db,
            engine,
            connection: Some(connection),
            committed: false,
            savepoint_counter: 0,
            pending_savepoint_rollback: None,
        })
    }

    /// Commit the transaction.
    pub async fn commit(mut self) -> Result<()> {
        let db_schema = self.engine.schema.db.clone();
        self.conn_mut()
            .exec(&db_schema, operation::Transaction::Commit.into())
            .await?;
        self.committed = true;
        Ok(())
    }

    /// Roll back the transaction.
    pub async fn rollback(mut self) -> Result<()> {
        let db_schema = self.engine.schema.db.clone();
        self.conn_mut()
            .exec(&db_schema, operation::Transaction::Rollback.into())
            .await?;
        self.committed = true;
        Ok(())
    }

    /// Create a savepoint within this transaction.
    pub async fn savepoint(&'db mut self) -> Result<Savepoint<'_>> {
        let id = self.savepoint_counter;
        self.savepoint_counter += 1;

        let db_schema = self.engine.schema.db.clone();
        self.conn_mut()
            .exec(&db_schema, operation::Transaction::Savepoint(id).into())
            .await?;

        Ok(Savepoint {
            parent: SavepointParent::Transaction(self),
            id,
            released: false,
        })
    }

    fn conn_mut(&mut self) -> &mut PoolConnection {
        self.connection
            .as_mut()
            .expect("connection taken after commit/rollback")
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.committed {
            if let Some(connection) = self.connection.take() {
                let db_schema = self.engine.schema.db.clone();
                tokio::spawn(async move {
                    let mut connection = connection;
                    let _ = connection
                        .exec(&db_schema, operation::Transaction::Rollback.into())
                        .await;
                });
            }
        }
    }
}

/// Helper enum to manage recursively defined savepoints.
enum SavepointParent<'a> {
    Transaction(&'a mut Transaction<'a>),
    Savepoint(&'a mut Savepoint<'a>),
}

impl<'a> SavepointParent<'a> {
    fn transaction_mut(&mut self) -> &mut Transaction<'a> {
        match self {
            Self::Transaction(inner) => inner,
            Self::Savepoint(inner) => inner.parent.transaction_mut(),
        }
    }
}

/// A savepoint within a transaction or another savepoint.
///
/// The mutable borrow of the parent enforces that only one savepoint is active
/// at a given nesting level.
///
/// If dropped without calling [`release`](Self::release) or
/// [`rollback`](Self::rollback), the savepoint is automatically rolled back.
pub struct Savepoint<'a> {
    parent: SavepointParent<'a>,
    id: usize,
    released: bool,
}

impl Savepoint<'_> {
    /// Commit (release) the savepoint. Changes become part of the parent scope.
    pub async fn release(mut self) -> Result<()> {
        let schema = self.parent.transaction_mut().engine.schema.db.clone();
        let connection = self.parent.transaction_mut().conn_mut();
        connection
            .exec(
                &schema,
                operation::Transaction::ReleaseSavepoint(self.id).into(),
            )
            .await?;
        self.released = true;
        Ok(())
    }

    /// Roll back to the savepoint. Undoes all work since the savepoint was created.
    pub async fn rollback(mut self) -> Result<()> {
        let schema = self.parent.transaction_mut().engine.schema.db.clone();
        let connection = self.parent.transaction_mut().conn_mut();
        connection
            .exec(
                &schema,
                operation::Transaction::RollbackToSavepoint(self.id).into(),
            )
            .await?;
        self.released = true;
        Ok(())
    }

    /// Create a nested savepoint.
    pub async fn savepoint(&mut self) -> Result<Savepoint<'_>> {
        let id = self.parent.transaction_mut().next_savepoint_id();
        let schema = self.parent.transaction_mut().engine.schema.db.clone();
        let connection = self.parent.transaction_mut().conn_mut();
        connection
            .exec(&schema, operation::Transaction::Savepoint(id).into())
            .await?;
        Ok(Savepoint {
            parent: SavepointParent::Savepoint(self),
            id,
            released: false,
        })
    }
}

impl Drop for Savepoint<'_> {
    fn drop(&mut self) {
        if !self.released {
            // We cannot do the asynchronous `RollbackToSavepoint` operation in the synchronous drop
            // method, so we notify the parent `Transaction` to do this for us before the next incoming
            // operation.
            self.parent.transaction_mut().pending_savepoint_rollback = Some(self.id);
        }
    }
}

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
