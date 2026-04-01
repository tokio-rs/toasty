use toasty_core::schema::app::{self, ModelId};

use crate::db::Builder;

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

/// An item discovered at compile time by the `#[derive(Model)]` or
/// `#[derive(Embed)]` macros.
///
/// Each derived type emits an `inventory::submit!` call that creates a
/// `DiscoverItem` carrying the originating crate name (via
/// `env!("CARGO_PKG_NAME")`) and a registration function. When the
/// `discover` feature is enabled, [`Builder::discover`] and
/// [`Builder::discover_crate`] iterate over all submitted items.
#[doc(hidden)]
pub struct DiscoverItem {
    crate_name: &'static str,
    register_fn: fn(&mut Builder),
}

impl DiscoverItem {
    pub const fn new(crate_name: &'static str, register_fn: fn(&mut Builder)) -> Self {
        Self {
            crate_name,
            register_fn,
        }
    }

    pub fn crate_name(&self) -> &'static str {
        self.crate_name
    }

    pub fn register(&self, builder: &mut Builder) {
        (self.register_fn)(builder)
    }
}

// Collect all `DiscoverItem` instances submitted by derive macros so they
// can be iterated by `Builder::discover` / `Builder::discover_crate`.
#[cfg(feature = "discover")]
inventory::collect!(DiscoverItem);

// Re-exported so that generated `inventory::submit!` calls can reference
// the crate without requiring users to depend on it directly.
#[cfg(feature = "discover")]
pub use inventory;
