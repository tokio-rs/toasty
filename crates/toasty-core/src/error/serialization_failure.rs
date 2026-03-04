use super::Error;

#[derive(Debug)]
pub(super) struct SerializationFailure {
    message: Box<str>,
}

impl std::error::Error for SerializationFailure {}

impl core::fmt::Display for SerializationFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "transaction serialization failure: {}", self.message)
    }
}

impl Error {
    /// Creates a serialization failure error.
    ///
    /// Returned when the database aborts a transaction due to a serialization
    /// conflict (e.g. PostgreSQL SQLSTATE 40001, MySQL error 1213).
    pub fn serialization_failure(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::SerializationFailure(
            SerializationFailure {
                message: message.into().into(),
            },
        ))
    }

    /// Returns `true` if this error is a serialization failure.
    pub fn is_serialization_failure(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::SerializationFailure(_))
    }
}
