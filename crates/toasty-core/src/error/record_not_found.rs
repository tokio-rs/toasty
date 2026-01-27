use super::Error;

/// Error when a record lookup (by query or key) returns no results.
#[derive(Debug)]
pub(super) struct RecordNotFound {
    context: Option<Box<str>>,
}

impl std::error::Error for RecordNotFound {}

impl core::fmt::Display for RecordNotFound {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("record not found")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}

impl Error {
    /// Creates a record not found error.
    ///
    /// This is the root cause error when a record lookup (by query or key) returns no results.
    ///
    /// The context parameter provides immediate context about what was not found.
    /// Additional context can be added at each layer via `.context()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// // With context describing what wasn't found (string literal)
    /// let err = Error::record_not_found("table=users key={id: 123}");
    /// assert_eq!(err.to_string(), "record not found: table=users key={id: 123}");
    ///
    /// // With context from format! or String
    /// let table = "users";
    /// let key = 123;
    /// let err = Error::record_not_found(format!("table={} key={}", table, key));
    /// assert_eq!(err.to_string(), "record not found: table=users key=123");
    /// ```
    pub fn record_not_found(context: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::RecordNotFound(RecordNotFound {
            context: Some(context.into().into()),
        }))
    }

    /// Returns `true` if this error is a record not found error.
    pub fn is_record_not_found(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::RecordNotFound(_))
    }
}
