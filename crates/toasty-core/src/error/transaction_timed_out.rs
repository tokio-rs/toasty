use std::time::Duration;

use crate::{error::ErrorKind, Error};

#[derive(Debug)]
pub(super) struct TransactionTimedOut {
    duration: Duration,
}

impl Error {
    pub fn transaction_timed_out(duration: Duration) -> Error {
        ErrorKind::TransactionTimedOut(TransactionTimedOut { duration }).into()
    }
}

impl std::error::Error for TransactionTimedOut {}

impl core::fmt::Display for TransactionTimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction timed out after {:?}", self.duration)
    }
}
