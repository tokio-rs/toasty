use super::Error;

/// Error indicating the underlying database connection is broken.
///
/// Drivers return this when the backend reports the socket is closed,
/// the session was killed, or another fatal connection-level fault has
/// occurred. The pool evicts the connection before the error reaches
/// the caller, so retrying on the same `Db` will pick up a fresh one.
///
/// Toasty does not retry the operation automatically: a write that
/// failed mid-flight may or may not have reached the server, and only
/// the caller knows whether the operation is safe to retry.
#[derive(Debug)]
pub(super) struct ConnectionLost {
    pub(super) inner: Box<dyn std::error::Error + Send + Sync>,
}

impl std::error::Error for ConnectionLost {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

impl core::fmt::Display for ConnectionLost {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "connection lost: ")?;
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
    /// Creates a connection-lost error from an underlying driver error.
    ///
    /// Drivers map their backend's "connection is gone" errors (a closed
    /// `tokio_postgres` socket, `mysql_async::Error::Io`, etc.) to this
    /// constructor so the connection pool can evict the dead connection
    /// and so user code can branch on
    /// [`is_connection_lost`](Error::is_connection_lost).
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// let io_err = std::io::Error::new(
    ///     std::io::ErrorKind::ConnectionReset,
    ///     "broken pipe",
    /// );
    /// let err = Error::connection_lost(io_err);
    /// assert!(err.is_connection_lost());
    /// ```
    pub fn connection_lost(err: impl std::error::Error + Send + Sync + 'static) -> Error {
        Error::from(super::ErrorKind::ConnectionLost(ConnectionLost {
            inner: Box::new(err),
        }))
    }

    /// Returns `true` if this error indicates the connection was lost.
    ///
    /// The pool has already evicted the underlying connection by the
    /// time this error reaches the caller. Operations may be safe to
    /// retry on the same `Db`, but only the caller knows whether the
    /// operation itself is idempotent.
    pub fn is_connection_lost(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::ConnectionLost(_))
    }
}
