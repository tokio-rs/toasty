use crate::Result;
use toasty_core::stmt::{self, Value};

/// Trait for types that can serve as the target of an update operation.
///
/// This trait is implemented by types that represent different update targets:
/// - Generated query struct: builds the update from its inner query, producing
///   `Update<List<Model>>` for multi-row updates
/// - `&mut Model`: builds the update from the model's primary key, producing
///   `Update<Model>` for single-row updates
///
/// The associated type `Returning` determines the statement return type.
pub trait UpdateTarget {
    /// The type parameter for the typed `Update<R>` statement.
    type Returning;

    /// Build the update statement by combining this target's selection with the
    /// provided assignments.
    ///
    /// For query-based targets, this takes the inner query (replacing it with a
    /// default) and wraps it in an `Update<List<Model>>`.
    /// For `&mut Model`, this builds an `Update<Model>` from the model's PK.
    fn to_update_stmt(
        &mut self,
        assignments: stmt::Assignments,
    ) -> crate::stmt::Update<Self::Returning>;

    /// Apply the result of an update operation.
    ///
    /// For query-based updates, this discards the values.
    /// For instance updates, this reloads the model from the first value.
    fn apply_result(self, values: Vec<Value>) -> Result<()>;
}
