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

/// Base trait for types that can be registered with the database schema.
///
/// This trait is implemented by both root models (via `Model`) and embedded
/// types (via `Embed`). It provides the minimal interface needed for schema
/// registration.
pub trait Register {
    /// Unique identifier for this type within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    fn id() -> ModelId;

    /// Returns the schema definition for this type.
    fn schema() -> app::Model;
}

/// Trait for root models that map to database tables and can be queried.
///
/// Root models have primary keys, can be queried independently, and support
/// full CRUD operations. They extend `Register` with queryability and
/// deserialization capabilities.
pub trait Model: Register + Sized {
    /// Query builder type for this model
    type Query;

    /// Create builder type for this model
    type Create;

    /// Update builder type for this model
    type Update<'a>;

    /// Update by query builder type for this model
    type UpdateQuery;

    /// Load an instance of the model from a value.
    ///
    /// The value is expected to be a `Value::Record` containing the model's fields.
    fn load(value: stmt::Value) -> Result<Self, Error>;
}

/// Trait for embedded types that are flattened into their parent model's table.
///
/// Embedded types don't have their own tables or primary keys. They can't be
/// queried independently or used as relation targets. Their fields are flattened
/// into the parent model's table columns.
pub trait Embed: Register {
    // Inherits id() and schema() from Register
    // No additional methods needed
}

// TODO: This is a hack to aid in the transition from schema code gen to proc
// macro. This should be removed once the proc macro is implemented.
impl<T: Model> Register for Option<T> {
    fn id() -> ModelId {
        T::id()
    }

    fn schema() -> app::Model {
        T::schema()
    }
}

impl<T: Model> Model for Option<T> {
    type Query = T::Query;
    type Create = T::Create;
    type Update<'a> = T::Update<'a>;
    type UpdateQuery = T::UpdateQuery;

    fn load(value: stmt::Value) -> Result<Self, Error> {
        Ok(Some(T::load(value)?))
    }
}
