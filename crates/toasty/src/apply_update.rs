use toasty_core::stmt::ValueStream;
use crate::Result;

/// Trait for types that can handle the result of an update operation.
///
/// This trait is implemented by types that represent different update targets:
/// - [`Query`]: Discards the result stream (for query-based updates)
/// - `&mut Model`: Reads from the stream and reloads the model (for instance updates)
pub trait ApplyUpdate {
    /// Apply the result of an update operation.
    ///
    /// For query-based updates, this discards the stream.
    /// For instance updates, this reloads the model from the first value in the stream.
    async fn apply_result(self, stream: ValueStream) -> Result<()>;
}

/// Marker type for query-based updates that don't reload a model instance.
#[derive(Debug, Clone, Copy)]
pub struct Query;

impl ApplyUpdate for Query {
    async fn apply_result(self, _stream: ValueStream) -> Result<()> {
        // Discard the stream - we don't need to reload anything
        Ok(())
    }
}
