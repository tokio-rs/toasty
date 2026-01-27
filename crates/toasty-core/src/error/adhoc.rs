use super::Error;

/// An ad-hoc error created from a format string.
#[derive(Debug)]
pub(super) struct AdhocError {
    pub(super) message: Box<str>,
}

impl AdhocError {
    pub(super) fn from_args<'a>(message: core::fmt::Arguments<'a>) -> AdhocError {
        use std::string::ToString;

        let message = message.to_string().into_boxed_str();
        AdhocError { message }
    }
}

impl std::error::Error for AdhocError {}

impl core::fmt::Display for AdhocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.message, f)
    }
}

impl Error {
    /// Creates an error from a format string.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// let err = Error::from_args(format_args!("value {} is invalid", "foo"));
    /// ```
    pub fn from_args<'a>(message: core::fmt::Arguments<'a>) -> Error {
        Error::from(super::ErrorKind::Adhoc(AdhocError::from_args(message)))
    }

    /// Returns `true` if this error is an adhoc error.
    pub fn is_adhoc(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::Adhoc(_))
    }
}
