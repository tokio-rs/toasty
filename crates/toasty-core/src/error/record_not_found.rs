/// Error when a record lookup (by query or key) returns no results.
#[derive(Debug)]
pub(super) struct RecordNotFoundError {
    pub(super) context: Option<Box<str>>,
}

impl RecordNotFoundError {
    pub(super) fn new(context: Option<Box<str>>) -> Self {
        RecordNotFoundError { context }
    }
}

impl std::error::Error for RecordNotFoundError {}

impl core::fmt::Display for RecordNotFoundError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str("record not found")?;
        if let Some(ref ctx) = self.context {
            write!(f, ": {}", ctx)?;
        }
        Ok(())
    }
}
