use toasty_core::Error;
use turso::Error as TursoError;

/// Classifies a [`turso::Error`] into a Toasty [`Error`].
///
/// * `Busy` and `BusySnapshot` — what the engine returns when a
///   `BEGIN CONCURRENT` transaction conflicts on commit, or when a writer
///   would have blocked. Both are retryable; map to
///   [`Error::serialization_failure`].
/// * `Error(msg)` containing the substring `"conflict"` — the current
///   `turso` crate (0.6) sometimes surfaces MVCC commit conflicts on this
///   generic variant rather than as `Busy*`. Its own
///   `examples/concurrent_writes.rs` checks the message text the same way;
///   treat it as retryable until upstream normalizes the variant.
/// * `Readonly` — the database refused a write because the connection is
///   in read-only mode. Map to [`Error::read_only_transaction`].
/// * `IoError` — a low-level I/O fault on the storage layer. Map to
///   [`Error::connection_lost`] so the pool evicts the slot.
/// * Everything else carries an opaque message; map to
///   [`Error::driver_operation_failed`].
pub(crate) fn classify_turso_error(err: TursoError) -> Error {
    match err {
        TursoError::Busy(msg) | TursoError::BusySnapshot(msg) => Error::serialization_failure(msg),
        TursoError::Error(msg) if msg.contains("conflict") => Error::serialization_failure(msg),
        TursoError::Readonly(msg) => Error::read_only_transaction(msg),
        TursoError::IoError(_, _) => Error::connection_lost(err),
        _ => Error::driver_operation_failed(err),
    }
}
