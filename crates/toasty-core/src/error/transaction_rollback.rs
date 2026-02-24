use super::Error;

#[derive(Debug)]
pub(super) struct TransactionRollback;

impl std::error::Error for TransactionRollback {}

impl core::fmt::Display for TransactionRollback {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "transaction rolled back")
    }
}

impl Error {
    /// Returns this error from a transaction closure to explicitly roll back.
    pub fn transaction_rollback() -> Error {
        Error::from(super::ErrorKind::TransactionRollback(TransactionRollback))
    }

    /// Returns `true` if this error is a deliberate transaction rollback.
    pub fn is_transaction_rollback(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::TransactionRollback(_))
    }
}
