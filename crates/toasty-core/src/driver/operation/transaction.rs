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

    /// Create a savepoint (nested transaction)
    Savepoint { depth: u32 },

    /// Release a savepoint (commit nested transaction)
    ReleaseSavepoint { depth: u32 },

    /// Rollback to a savepoint (rollback nested transaction)
    RollbackToSavepoint { depth: u32 },
}

impl From<Transaction> for Operation {
    fn from(value: Transaction) -> Self {
        Self::Transaction(value)
    }
}
