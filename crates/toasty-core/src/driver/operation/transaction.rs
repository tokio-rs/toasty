use super::Operation;

#[derive(Debug)]
pub enum Transaction {
    /// Start a transaction
    Start,

    /// Commit a transaction
    Commit,

    /// Rollback a transaction
    Rollback,
}

impl From<Transaction> for Operation {
    fn from(value: Transaction) -> Self {
        Self::Transaction(value)
    }
}
