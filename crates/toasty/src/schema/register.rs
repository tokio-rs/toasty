use toasty_core::schema::app::{self, ModelId, ModelSet};

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
/// `env!("CARGO_PKG_NAME")`) and a registration function.
/// [`toasty::models!`] iterates over all submitted items filtered by crate
/// name.
#[doc(hidden)]
pub struct DiscoverItem {
    crate_name: &'static str,
    add_fn: fn(&mut ModelSet),
}

impl DiscoverItem {
    pub const fn new(crate_name: &'static str, add_fn: fn(&mut ModelSet)) -> Self {
        Self { crate_name, add_fn }
    }

    pub fn crate_name(&self) -> &'static str {
        self.crate_name
    }

    pub fn add_to(&self, model_set: &mut ModelSet) {
        (self.add_fn)(model_set)
    }

    pub fn add_all_from_crate_to(model_set: &mut ModelSet, crate_name: &str) {
        // Normalize all crate names to use `_` instead of `-` for `models!` invocation
        // which has to use `_` to be a valid crate identifier.
        let crate_name = crate_name.replace("-", "_");
        for item in inventory::iter::<Self> {
            if item.crate_name().replace("-", "_") == crate_name {
                item.add_to(model_set);
            }
        }
    }
}

// Collect all `DiscoverItem` instances submitted by derive macros so they
// can be iterated by [`toasty::models!`].
inventory::collect!(DiscoverItem);

// Re-exported so that generated `inventory::submit!` calls can reference
// the crate without requiring users to depend on it directly.
pub use inventory;

/// Creates a [`ModelSet`] containing the specified models.
///
/// The resulting `ModelSet` is typically passed to
/// [`Builder::models`](crate::Db::builder) when setting up a database
/// connection:
///
/// ```ignore
/// let db = toasty::Db::builder()
///     .models(toasty::models!(User, Todo))
///     .connect("sqlite::memory:")
///     .await?;
/// ```
///
/// # Syntax
///
/// The macro accepts a comma-separated list of any combination of:
///
/// - **Individual models** — a type path to a struct that derives `Model` or
///   `Embed`. Module paths are supported (e.g. `my_module::MyModel`).
/// - **`crate::*`** — registers every model discovered in the current crate.
/// - **`some_crate::*`** — registers every model discovered in the named
///   external crate.
///
/// These forms can be freely combined:
///
/// ```ignore
/// toasty::models!(
///     // All models from the current crate
///     crate::*,
///     // All models from an external crate
///     third_party_models::*,
///     // Individual models, with or without a module path
///     MyModel,
///     other::SpecificModel,
/// )
/// ```
#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! models {
    // Register all models from current crate with `models!(crate::*)`
    (@internal $set:ident crate::* $(,$rest:ty)* $(,)?) => {{
        ::toasty::codegen_support::DiscoverItem::add_all_from_crate_to(&mut $set, env!("CARGO_PKG_NAME"));
        $crate::models!(@internal $set $($rest),*);
    }};

    // Register all models from a third party crate with `models!(third_party::*)`
    (@internal $set:ident $crate_name:ident::* $(,$rest:ty)* $(,)?) => {{
        // Make sure the provided crate actually exists.
        { use ::$crate_name; }
        ::toasty::codegen_support::DiscoverItem::add_all_from_crate_to(&mut $set, stringify!($crate_name));
        $crate::models!(@internal $set $($rest),*);
    }};

    // Register single model with `models!(ModelName)`
    (@internal $set:ident $model:ty $(,$rest:ty)* $(,)?) => {{
        $set.add(<$model as ::toasty::schema::Register>::schema());
        $crate::models!(@internal $set $($rest),*);
    }};

    // Empty list
    (@internal $set:ident) => {};

    ($($tokens:tt)*) => {{
        let mut model_set = ::toasty::schema::ModelSet::new();
        $crate::models!(@internal model_set $($tokens)*);
        model_set
    }};
}
