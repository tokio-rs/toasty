use super::Error;

/// An ad-hoc error created from a format string.
#[derive(Debug)]
pub(super) struct Adhoc {
    pub(super) message: Box<str>,
}

impl Adhoc {
    pub(super) fn from_args<'a>(message: core::fmt::Arguments<'a>) -> Adhoc {
        use std::string::ToString;

        let message = message.to_string().into_boxed_str();
        Adhoc { message }
    }
}

impl std::error::Error for Adhoc {}

impl core::fmt::Display for Adhoc {
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
        Error::from(super::ErrorKind::Adhoc(Adhoc::from_args(message)))
    }

    /// Returns `true` if this error is an adhoc error.
    pub fn is_adhoc(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::Adhoc(_))
    }
}
