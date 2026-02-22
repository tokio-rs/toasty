use super::Error;

/// Error when a statement is invalid.
///
/// This occurs when:
/// - A statement references a non-existent field
/// - A statement contains invalid operations for a given field type
/// - A statement has incorrect structure or arguments
///
/// These errors are caught during statement lowering/execution, at runtime.
/// This is distinct from schema validation errors which occur at build/migration time.
#[derive(Debug)]
pub(super) struct InvalidStatement {
    pub(super) message: Box<str>,
}

impl std::error::Error for InvalidStatement {}

impl core::fmt::Display for InvalidStatement {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "invalid statement: {}", self.message)
    }
}

impl Error {
    /// Creates an invalid statement error.
    ///
    /// This is used when a statement is malformed or references invalid schema elements.
    /// These errors occur during statement lowering/execution at runtime.
    pub fn invalid_statement(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidStatement(InvalidStatement {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is an invalid statement error.
    pub fn is_invalid_statement(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidStatement(_))
    }
}
