use super::pool::{ConnectionHandle, Manager};

/// A connection retrieved from a pool.
///
/// When dropped, the connection is returned to the pool for reuse.
pub struct Connection {
    pub(super) inner: deadpool::managed::Object<Manager>,
}

impl Connection {
    /// Access the underlying connection handle.
    pub(crate) fn handle(&self) -> &ConnectionHandle {
        &self.inner
    }
}
