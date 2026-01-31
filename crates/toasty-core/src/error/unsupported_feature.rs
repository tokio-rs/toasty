use super::Error;

/// Error when a database does not support a requested feature.
///
/// This occurs when:
/// - A statement type is not supported by the database (unsupported primitive type)
/// - A storage type is not available (VARCHAR not supported)
/// - A feature constraint is exceeded (VARCHAR size exceeds database limit)
///
/// These errors are typically caught during schema construction or validation,
/// indicating a mismatch between application requirements and database capabilities.
#[derive(Debug)]
pub(super) struct UnsupportedFeature {
    message: Box<str>,
}

impl std::error::Error for UnsupportedFeature {}

impl core::fmt::Display for UnsupportedFeature {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "unsupported feature: {}", self.message)
    }
}

impl Error {
    /// Creates an unsupported feature error.
    ///
    /// This is used when a database does not support a requested feature,
    /// such as a specific type, storage constraint, or capability.
    pub fn unsupported_feature(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::UnsupportedFeature(UnsupportedFeature {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is an unsupported feature error.
    pub fn is_unsupported_feature(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::UnsupportedFeature(_))
    }
}
