use super::{Load, Register};
use crate::stmt::{IntoExpr, IntoInsert};

/// Trait for root models that map to database tables and can be queried.
///
/// Root models have primary keys, can be queried independently, and support
/// full CRUD operations. They extend `Register` with queryability and
/// deserialization capabilities.
pub trait Model: Register + Load<Output = Self> + Sized {
    /// Query builder type for this model
    type Query;

    /// Create builder type for this model
    type Create: Default + IntoInsert<Model = Self> + IntoExpr<Self>;

    /// Update builder type for this model
    type Update<'a>;

    /// Update by query builder type for this model
    type UpdateQuery;

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }
}
