use super::Error;

/// Error when a driver's capability configuration is invalid or inconsistent.
///
/// This occurs when:
/// - Driver capability flags are inconsistent (e.g., native_varchar=true but varchar type not specified)
/// - Required capability fields are missing or contradictory
///
/// These errors indicate a programming error in the driver implementation itself,
/// not a user error or runtime condition.
#[derive(Debug)]
pub(super) struct InvalidDriverConfiguration {
    message: Box<str>,
}

impl std::error::Error for InvalidDriverConfiguration {}

impl core::fmt::Display for InvalidDriverConfiguration {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "invalid driver configuration: {}", self.message)
    }
}

impl Error {
    /// Creates an invalid driver configuration error.
    ///
    /// This is used when a driver's capability configuration is invalid or inconsistent.
    /// These errors indicate a bug in the driver implementation.
    pub fn invalid_driver_configuration(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidDriverConfiguration(
            InvalidDriverConfiguration {
                message: message.into().into(),
            },
        ))
    }

    /// Returns `true` if this error is an invalid driver configuration error.
    pub fn is_invalid_driver_configuration(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidDriverConfiguration(_))
    }
}
