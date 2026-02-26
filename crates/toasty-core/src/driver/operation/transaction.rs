use super::Operation;

/// Isolation levels supported across SQL backends.
///
/// Not all backends support all levels — the driver will return an error
/// if an unsupported level is requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone)]
pub enum Transaction {
    /// Start a transaction with optional configuration.
    ///
    /// When `isolation` is `None` and `read_only` is `false`, the database's
    /// default isolation level and read-write mode are used.
    Start {
        isolation: Option<IsolationLevel>,
        read_only: bool,
    },

    /// Commit a transaction
    Commit,

    /// Rollback a transaction
    Rollback,

    /// Create a savepoint with the given numeric identifier
    Savepoint(usize),

    /// Release (commit) a savepoint
    ReleaseSavepoint(usize),

    /// Rollback to a savepoint, undoing work since it was created
    RollbackToSavepoint(usize),
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

impl From<Transaction> for Operation {
    fn from(value: Transaction) -> Self {
        Self::Transaction(value)
    }
}
