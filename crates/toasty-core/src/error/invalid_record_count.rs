use super::Error;

/// Error when an operation expects exactly one record but finds multiple.
///
/// This occurs when:
/// - A query that should return one record returns multiple
/// - An operation explicitly requires a single result but gets more
#[derive(Debug)]
pub(super) struct InvalidRecordCount {
    context: Option<Box<str>>,
}

impl std::error::Error for InvalidRecordCount {}

impl core::fmt::Display for InvalidRecordCount {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("invalid record count")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}

impl Error {
    /// Creates an invalid record count error.
    ///
    /// This is used when an operation expects exactly one record but finds multiple.
    ///
    /// The context parameter provides information about the operation.
    pub fn invalid_record_count(context: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::InvalidRecordCount(InvalidRecordCount {
            context: Some(context.into().into()),
        }))
    }

    /// Returns `true` if this error is an invalid record count error.
    pub fn is_invalid_record_count(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::InvalidRecordCount(_))
    }
}
