/// Error when a conditional operation's condition evaluates to false.
///
/// This occurs when:
/// - An UPDATE with a WHERE clause matches no rows (condition didn't match)
/// - A DynamoDB conditional write fails (ConditionalCheckFailedException)
/// - An optimistic lock version check fails
#[derive(Debug)]
pub(super) struct ConditionFailedError {
    /// Optional context describing what condition failed
    pub(super) context: Option<Box<str>>,
}

impl ConditionFailedError {
    pub(super) fn new(context: Option<Box<str>>) -> Self {
        ConditionFailedError { context }
    }
}

impl std::error::Error for ConditionFailedError {}

impl core::fmt::Display for ConditionFailedError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("condition failed")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}
