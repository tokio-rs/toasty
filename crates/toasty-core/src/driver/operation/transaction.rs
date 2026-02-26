use super::Operation;
use crate::driver::transaction::IsolationLevel;

#[derive(Debug, Clone)]
pub enum Transaction {
    /// Start a transaction
    Start { isolation: Option<IsolationLevel> },

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

impl From<Transaction> for Operation {
    fn from(value: Transaction) -> Self {
        Self::Transaction(value)
    }
}
