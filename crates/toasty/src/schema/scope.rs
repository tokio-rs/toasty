use crate::stmt::Path;

/// A scope represents a context that contains items of a particular type.
///
/// Generated query types are scopes whose items are instances of the target
/// model. The trait provides associated types for building typed paths and
/// create builders within the scope.
///
/// Whether a particular scope can satisfy a scoped create is decided at
/// execution time: `create_in_scope` always produces a create builder and
/// forwards the scope to the engine, which validates whether it can populate
/// the required fields (for example, by reading a foreign key from a single-
/// step relation traversal).
#[diagnostic::on_unimplemented(
    message = "this expression cannot be used as a Toasty create scope",
    label = "this expression does not support scoped creation"
)]
pub trait Scope {
    /// The item type contained in this scope.
    type Item;

    /// A typed path from `Origin` into this scope.
    type Path<Origin>;

    /// The create builder for items in this scope.
    type Create;

    /// Construct a scope path from a [`Path`] targeting the item type.
    fn new_path<Origin>(path: Path<Origin, Self::Item>) -> Self::Path<Origin>;

    /// Return a fresh, default-initialized create builder for this scope.
    fn new_create() -> Self::Create;

    /// Return a root path for this scope, anchored at the scope's model.
    ///
    /// This is used by the `create!` macro to obtain field accessors for
    /// nested builders without needing to know the concrete model type.
    fn new_path_root() -> Self::Path<Self::Item>;

    /// Build a create builder scoped to this expression. Forwards any
    /// relation-association metadata to the engine, which decides at exec
    /// time whether the scope can be satisfied.
    fn create_in_scope(self) -> Self::Create;
}
