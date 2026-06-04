use super::Operation;

/// SQL transaction isolation levels.
///
/// Not all backends support all levels. The driver returns an error if an
/// unsupported level is requested.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::operation::IsolationLevel;
///
/// let level = IsolationLevel::Serializable;
/// assert_eq!(level, IsolationLevel::Serializable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Transactions can see uncommitted changes from other transactions.
    ReadUncommitted,
    /// Transactions only see changes committed before each statement.
    ReadCommitted,
    /// Transactions see a consistent snapshot from the start of the transaction.
    RepeatableRead,
    /// Full serializability; transactions behave as if executed one at a time.
    Serializable,
}

/// How a transaction acquires write locks.
///
/// Orthogonal to [`IsolationLevel`]: an isolation level describes *what
/// anomalies* a transaction can observe; a mode describes *when* the
/// transaction acquires its locks.
///
/// Only SQLite (and SQLite-compatible engines) currently expose this
/// dimension to clients:
///
/// * [`Default`](Self::Default) → whatever the driver picks. For SQLite
///   that is `BEGIN` (DEFERRED) today; for a future driver it may not
///   be — e.g. Turso under MVCC plans to default to `BEGIN CONCURRENT`.
/// * [`Deferred`](Self::Deferred) → `BEGIN` (DEFERRED): explicit
///   deferred locking. Identical to `Default` on plain SQLite; on a
///   driver whose default is *not* deferred (Turso MVCC), this is how
///   the caller opts out of that default.
/// * [`Immediate`](Self::Immediate) → `BEGIN IMMEDIATE`: write lock
///   acquired up front, so a later write inside the transaction cannot
///   fail with `SQLITE_BUSY`.
/// * [`Exclusive`](Self::Exclusive) → `BEGIN EXCLUSIVE`: exclusive lock
///   held for the lifetime of the transaction; no other connection —
///   reader or writer — can make progress against the database file.
///
/// Drivers that do not implement a given mode return
/// [`Error::unsupported_feature`](crate::Error::unsupported_feature) when
/// the transaction starts. Future drivers may extend this enum (e.g. a
/// Turso `Concurrent` variant for `BEGIN CONCURRENT` under MVCC).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransactionMode {
    /// The driver's natural default. May differ from
    /// [`Deferred`](Self::Deferred) on drivers that prefer a different
    /// locking strategy (e.g. Turso MVCC → `BEGIN CONCURRENT`).
    #[default]
    Default,
    /// Explicit deferred locking. SQLite → `BEGIN` (DEFERRED). Use this
    /// to override a driver whose `Default` is not deferred.
    Deferred,
    /// Acquire a write lock when the transaction begins. SQLite →
    /// `BEGIN IMMEDIATE`. Rejected by drivers without an equivalent.
    Immediate,
    /// Hold an exclusive lock for the entire transaction. SQLite →
    /// `BEGIN EXCLUSIVE`. Rejected by drivers without an equivalent.
    Exclusive,
}

/// A transaction lifecycle operation.
///
/// Covers the full transaction lifecycle: begin, commit, rollback, and
/// savepoint management. Convert to [`Operation`] with `.into()`.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::operation::{Transaction, Operation};
///
/// // Start a default transaction
/// let op: Operation = Transaction::start().into();
///
/// // Commit
/// let op: Operation = Transaction::Commit.into();
/// assert!(op.is_transaction_commit());
/// ```
#[derive(Debug, Clone)]
pub enum Transaction {
    /// Start a transaction with optional configuration.
    ///
    /// When `isolation` is `None`, `read_only` is `false`, and `mode` is
    /// [`TransactionMode::Default`], the database's natural defaults are
    /// used.
    Start {
        /// Optional isolation level. `None` uses the database default.
        isolation: Option<IsolationLevel>,
        /// When `true`, the transaction is read-only.
        read_only: bool,
        /// Lock-acquisition mode. See [`TransactionMode`].
        mode: TransactionMode,
    },

    /// Commit a transaction
    Commit,

    /// Rollback a transaction
    Rollback,

    /// Create a savepoint with the given identifier
    Savepoint(String),

    /// Release (commit) a savepoint
    ReleaseSavepoint(String),

    /// Rollback to a savepoint, undoing work since it was created
    RollbackToSavepoint(String),
}

impl Transaction {
    /// Start a transaction with database defaults.
    pub fn start() -> Self {
        Self::Start {
            isolation: None,
            read_only: false,
            mode: TransactionMode::Default,
        }
    }
}

impl Operation {
    /// Returns `true` if this is a [`Transaction::Commit`] operation.
    pub fn is_transaction_commit(&self) -> bool {
        matches!(self, Operation::Transaction(Transaction::Commit))
    }
}

impl From<Transaction> for Operation {
    fn from(value: Transaction) -> Self {
        Self::Transaction(value)
    }
}
