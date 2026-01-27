use super::Error;

/// Error when an operation expects exactly one record but finds multiple.
///
/// This occurs when:
/// - A query that should return one record returns multiple
/// - An operation explicitly requires a single result but gets more
#[derive(Debug)]
pub(super) struct TooManyRecordsError {
    context: Option<Box<str>>,
}

impl std::error::Error for TooManyRecordsError {}

impl core::fmt::Display for TooManyRecordsError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("too many records")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}

impl Error {
    /// Creates a too many records error.
    ///
    /// This is used when an operation expects exactly one record but finds multiple.
    ///
    /// The context parameter provides information about the operation.
    pub fn too_many_records(context: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::TooManyRecords(TooManyRecordsError {
            context: Some(context.into().into()),
        }))
    }

    /// Returns `true` if this error is a too many records error.
    pub fn is_too_many_records(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::TooManyRecords(_))
    }
}
