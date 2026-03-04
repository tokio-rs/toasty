use std::time::Duration;

use crate::{error::ErrorKind, Error};

#[derive(Debug)]
pub(super) struct TransactionTimeout {
    duration: Duration,
}

impl Error {
    /// Returned when the transaction closure exceeds the configured timeout.
    /// The transaction is automatically rolled back.
    pub fn transaction_timeout(duration: Duration) -> Error {
        ErrorKind::TransactionTimeout(TransactionTimeout { duration }).into()
    }

    /// Returns `true` if this error is a transaction timeout.
    pub fn is_transaction_timeout(&self) -> bool {
        matches!(self.kind(), ErrorKind::TransactionTimeout(_))
    }
}

impl std::error::Error for TransactionTimeout {}

impl core::fmt::Display for TransactionTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction timed out after {:?}", self.duration)
    }
}
