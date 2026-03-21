use std::time::Duration;

use crate::{error::ErrorKind, Error};

/// Error when a transaction exceeds its configured timeout.
///
/// The transaction is automatically rolled back when this occurs.
#[derive(Debug)]
pub(super) struct TransactionTimeout {
    duration: Duration,
}

impl Error {
    /// Creates a transaction timeout error.
    ///
    /// Returned when the transaction closure exceeds the configured timeout.
    /// The transaction is automatically rolled back.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use toasty_core::Error;
    ///
    /// let err = Error::transaction_timeout(Duration::from_secs(30));
    /// assert!(err.is_transaction_timeout());
    /// ```
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
