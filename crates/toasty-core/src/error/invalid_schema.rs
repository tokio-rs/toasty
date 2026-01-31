use super::Error;

/// Error when a schema definition is invalid.
///
/// This occurs when:
/// - A schema has duplicate names (index names, etc.)
/// - A column configuration is invalid (auto_increment on non-numeric type)
/// - Incompatible features are combined (auto_increment with composite keys)
/// - Required constraints are violated (auto_increment must be in primary key)
///
/// These errors are caught during schema construction/validation, typically at build time.
#[derive(Debug)]
pub(super) struct InvalidSchema {
    message: Box<str>,
}

impl std::error::Error for InvalidSchema {}

impl core::fmt::Display for InvalidSchema {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "invalid schema: {}", self.message)
    }
}

impl Error {
    /// Creates an invalid schema error.
    ///
    /// This is used when a schema definition is invalid - duplicate names,
    /// invalid column configurations, incompatible features, etc.
    /// These errors are typically caught at build/migration time.
    pub fn invalid_schema(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidSchema(InvalidSchema {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is an invalid schema error.
    pub fn is_invalid_schema(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidSchema(_))
    }
}
