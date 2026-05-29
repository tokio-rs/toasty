use toasty_core::schema::app::{self, ModelId};

/// Trait for embedded types that are flattened into their parent model's table.
///
/// Embedded types don't have their own tables or primary keys. They can't be
/// queried independently or used as relation targets. Their fields are flattened
/// into the parent model's table columns.
///
/// Embedded types are never registered directly by the user — they are
/// discovered transitively through the fields of the models (and other embeds)
/// that contain them, via [`Field::register`](super::Field::register).
pub trait Embed {
    /// Unique identifier for this embedded type within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    fn id() -> ModelId;

    /// Returns the schema definition for this embedded type.
    fn schema() -> app::Model;
}
