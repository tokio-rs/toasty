use crate::stmt::List;
use crate::Result;
use std::marker::PhantomData;
use toasty_core::stmt::Value;

/// Trait for types that can handle the result of an update operation.
///
/// This trait is implemented by types that represent different update targets:
/// - [`Query`]: Discards the result values (for query-based updates)
/// - `&mut Model`: Reads the first value and reloads the model (for instance updates)
///
/// The associated type `Returning` determines the statement return type:
/// - `List<Model>` for query-based updates (multiple rows)
/// - `Model` for instance updates (single row)
pub trait UpdateTarget {
    /// The type parameter for the typed `Update<R>` statement.
    type Returning;

    /// Apply the result of an update operation.
    ///
    /// For query-based updates, this discards the values.
    /// For instance updates, this reloads the model from the first value.
    fn apply_result(self, values: Vec<Value>) -> Result<()>;
}

/// Marker type for query-based updates that don't reload a model instance.
#[derive(Debug, Clone, Copy)]
pub struct Query<M>(PhantomData<M>);

impl<M> Default for Query<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M> Query<M> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<M> UpdateTarget for Query<M> {
    type Returning = List<M>;

    fn apply_result(self, _values: Vec<Value>) -> Result<()> {
        // Discard the values - we don't need to reload anything
        Ok(())
    }
}
