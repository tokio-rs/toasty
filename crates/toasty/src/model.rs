use crate::Error;
use toasty_core::{
    schema::app::{self, ModelId},
    stmt,
};

/// Generate a unique model ID at runtime.
///
/// This function uses a global atomic counter to ensure each call returns
/// a unique ModelId. IDs start at 0 and increment with each call.
/// This is thread-safe and can be called concurrently.
pub fn generate_unique_id() -> ModelId {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_MODEL_ID: AtomicUsize = AtomicUsize::new(0);

    let id = NEXT_MODEL_ID.fetch_add(1, Ordering::Relaxed);
    ModelId(id)
}

pub trait Model: Sized {
    /// Query builder type for this model
    type Query;

    /// Create builder type for this model
    type Create;

    /// Update builder type for this model
    type Update<'a>;

    /// Update by query builder type for this model
    type UpdateQuery;

    /// Unique identifier for this model within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    fn id() -> ModelId;

    /// Load an instance of the model, populating fields using the given row.
    fn load(row: stmt::ValueRecord) -> Result<Self, Error>;

    fn schema() -> app::Model {
        todo!()
    }
}

// TODO: This is a hack to aid in the transition from schema code gen to proc
// macro. This should be removed once the proc macro is implemented.
impl<T: Model> Model for Option<T> {
    type Query = T::Query;
    type Create = T::Create;
    type Update<'a> = T::Update<'a>;
    type UpdateQuery = T::UpdateQuery;

    fn id() -> ModelId {
        T::id()
    }

    fn load(row: stmt::ValueRecord) -> Result<Self, Error> {
        Ok(Some(T::load(row)?))
    }

    fn schema() -> app::Model {
        todo!()
    }
}
