use crate::Error;

/// Error when a database connection URL is malformed or invalid.
///
/// This occurs when a connection string cannot be parsed or contains
/// invalid parameters (e.g., unknown scheme, missing host, bad port).
#[derive(Debug)]
pub(super) struct InvalidConnectionUrl {
    pub(super) message: Box<str>,
}

impl Error {
    /// Creates an invalid connection URL error.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// let err = Error::invalid_connection_url("missing host in connection string");
    /// assert!(err.is_invalid_connection_url());
    /// assert_eq!(
    ///     err.to_string(),
    ///     "invalid connection URL: missing host in connection string"
    /// );
    /// ```
    pub fn invalid_connection_url(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidConnectionUrl(
            InvalidConnectionUrl {
                message: message.into().into(),
            },
        ))
    }

    /// Returns `true` if this error is an invalid connection URL error.
    pub fn is_invalid_connection_url(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidConnectionUrl(_))
    }
}

impl std::fmt::Display for InvalidConnectionUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid connection URL: {}", self.message)
    }
}
