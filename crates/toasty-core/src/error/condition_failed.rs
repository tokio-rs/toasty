use super::Error;

/// Error when a conditional operation's condition evaluates to false.
///
/// This occurs when:
/// - An UPDATE with a WHERE clause matches no rows (condition didn't match)
/// - A DynamoDB conditional write fails (ConditionalCheckFailedException)
/// - An optimistic lock version check fails
#[derive(Debug)]
pub(super) struct ConditionFailed {
    context: Option<Box<str>>,
}

impl std::error::Error for ConditionFailed {}

impl core::fmt::Display for ConditionFailed {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("condition failed")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}

impl Error {
    /// Creates a condition failed error.
    ///
    /// This is used when a conditional operation's condition evaluates to false, such as:
    /// - An UPDATE with a WHERE clause that matches no rows
    /// - A DynamoDB conditional write that fails
    /// - An optimistic lock version check that fails
    ///
    /// The context parameter provides information about what condition failed.
    pub fn condition_failed(context: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::ConditionFailed(ConditionFailed {
            context: Some(context.into().into()),
        }))
    }

    /// Returns `true` if this error is a condition failed error.
    pub fn is_condition_failed(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::ConditionFailed(_))
    }
}
