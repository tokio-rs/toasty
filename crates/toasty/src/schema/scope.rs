use crate::stmt::Path;

/// A scope represents a context that contains items of a particular type.
///
/// For example, a `HasMany<T>` is a scope whose items are instances of the
/// target model. The trait provides associated types for building typed paths
/// and create builders within the scope.
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

    /// Return the fields path for this scope, rooted at the scope's model.
    ///
    /// This is used by the `create!` macro to obtain field accessors for
    /// nested builders without needing to know the concrete model type.
    fn fields() -> Self::Path<Self::Item>;
}
