use super::Error;

#[derive(Debug)]
pub(super) struct ReadOnlyTransaction {
    message: Box<str>,
}

impl std::error::Error for ReadOnlyTransaction {}

impl core::fmt::Display for ReadOnlyTransaction {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "read-only transaction: {}", self.message)
    }
}

impl Error {
    /// Creates a read-only transaction error.
    ///
    /// Returned when a write operation is attempted inside a read-only
    /// transaction (e.g. PostgreSQL SQLSTATE 25006, MySQL error 1792).
    pub fn read_only_transaction(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::ReadOnlyTransaction(ReadOnlyTransaction {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is a read-only transaction error.
    pub fn is_read_only_transaction(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::ReadOnlyTransaction(_))
    }
}
