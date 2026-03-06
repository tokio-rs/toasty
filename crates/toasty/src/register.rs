use toasty_core::schema::app::{self, ModelId};

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
