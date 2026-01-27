use super::Error;

/// Error from a database driver.
#[derive(Debug)]
pub(super) struct DriverError {
    pub(super) inner: Box<dyn std::error::Error + Send + Sync>,
}

impl std::error::Error for DriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

impl core::fmt::Display for DriverError {
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
    /// Creates an error from a driver error.
    ///
    /// This is the preferred way to convert driver-specific errors (rusqlite, tokio-postgres,
    /// mysql_async, AWS SDK errors, etc.) into toasty errors.
    pub fn driver(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(super::ErrorKind::Driver(DriverError {
            inner: Box::new(err),
        }))
    }

    /// Returns `true` if this error is a driver error.
    pub fn is_driver(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::Driver(_))
    }
}
