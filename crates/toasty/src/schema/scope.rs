use crate::{schema::ValidateCreate, stmt::Path};

/// A scope represents a context that contains items of a particular type.
///
/// Generated relation query types are scopes whose items are instances of the
/// target model. The trait provides associated types for building typed paths
/// and create builders within the scope.
#[diagnostic::on_unimplemented(
    message = "this expression cannot be used as a Toasty create scope",
    label = "this expression does not support scoped creation",
    note = "Only direct relation scopes support scoped creation.",
    note = "Multi-step (`via`) relation scopes can be queried and filtered, but Toasty cannot create records through them because that would require creating or choosing intermediate records."
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
}

/// A scope that supports creating records inside the scope.
///
/// This is intentionally narrower than [`Scope`]. A multi-step (`via`) relation
/// has field accessors and can be queried, but it cannot safely create records
/// because there is no direct foreign key for Toasty to populate.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "this scope does not support scoped creation",
    label = "this relation scope cannot create records",
    note = "Only direct relation scopes support scoped creation.",
    note = "Multi-step (`via`) relation scopes can be queried and filtered, but Toasty cannot create records through them because that would require creating or choosing intermediate records."
)]
pub trait CreateScope: Scope + ValidateCreate {
    /// Create a record inside this scope.
    fn create_in_scope(self) -> Self::Create;
}
