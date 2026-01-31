use super::Error;

/// Error when a database driver operation fails.
///
/// This wraps errors from underlying database driver libraries when operations fail:
/// - Connection errors (rusqlite, tokio-postgres, AWS SDK)
/// - Query execution errors
/// - Transaction operation errors (BEGIN, COMMIT, ROLLBACK)
/// - Schema operation errors (CREATE TABLE, CREATE INDEX)
/// - URL parsing errors for connection strings
#[derive(Debug)]
pub(super) struct DriverOperationFailed {
    pub(super) inner: Box<dyn std::error::Error + Send + Sync>,
}

impl std::error::Error for DriverOperationFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

impl core::fmt::Display for DriverOperationFailed {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // Display the error and walk its source chain
        core::fmt::Display::fmt(&self.inner, f)?;
        let mut source = self.inner.source();
        while let Some(err) = source {
            write!(f, ": {}", err)?;
            source = err.source();
        }
        Ok(())
    }
}

impl Error {
    /// Creates an error from a driver operation failure.
    ///
    /// This is the preferred way to convert driver-specific errors (rusqlite, tokio-postgres,
    /// mysql_async, AWS SDK errors, etc.) into toasty errors.
    pub fn driver_operation_failed(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(super::ErrorKind::DriverOperationFailed(
            DriverOperationFailed {
                inner: Box::new(err),
            },
        ))
    }

    /// Returns `true` if this error is a driver operation failure.
    pub fn is_driver_operation_failed(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::DriverOperationFailed(_))
    }
}
