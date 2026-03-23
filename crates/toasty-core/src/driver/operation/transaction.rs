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
    /// When `isolation` is `None` and `read_only` is `false`, the database's
    /// default isolation level and read-write mode are used.
    Start {
        /// Optional isolation level. `None` uses the database default.
        isolation: Option<IsolationLevel>,
        /// When `true`, the transaction is read-only.
        read_only: bool,
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
