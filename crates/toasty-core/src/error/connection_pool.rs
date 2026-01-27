use super::Error;

/// Error from a connection pool.
#[derive(Debug)]
pub(super) struct ConnectionPoolError {
    pub(super) inner: Box<dyn std::error::Error + Send + Sync>,
}

impl std::error::Error for ConnectionPoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

impl core::fmt::Display for ConnectionPoolError {
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
    /// Creates an error from a connection pool error.
    ///
    /// This is used for errors that occur when managing the connection pool (e.g., deadpool errors).
    pub fn connection_pool(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(super::ErrorKind::ConnectionPool(ConnectionPoolError {
            inner: Box::new(err),
        }))
    }
}
