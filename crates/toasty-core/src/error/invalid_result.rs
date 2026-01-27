use super::Error;

/// Error when a query result has an unexpected structure.
///
/// This occurs when:
/// - A query returns a different result type than expected (Count vs Stream)
/// - A row value has an unexpected type (expected Record, got something else)
/// - A field has an unexpected type (expected I64, got something else)
///
/// This indicates the database returned valid data, but its structure doesn't match
/// what the query operation expected based on the schema and query type.
#[derive(Debug)]
pub(super) struct InvalidResult {
    message: Box<str>,
}

impl std::error::Error for InvalidResult {}

impl core::fmt::Display for InvalidResult {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "invalid result: {}", self.message)
    }
}

impl Error {
    /// Creates an invalid result error.
    ///
    /// This is used when a query result has an unexpected structure - the database
    /// returned valid data, but its shape doesn't match what the operation expected.
    pub fn invalid_result(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidResult(InvalidResult {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is an invalid result error.
    pub fn is_invalid_result(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidResult(_))
    }
}
