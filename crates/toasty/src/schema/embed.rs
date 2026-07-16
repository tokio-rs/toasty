use crate::stmt::Path;
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

    /// An identity [`Path`] rooted at this embedded type.
    ///
    /// This is how `Path` recovers an embedded type's [`ModelId`] without a
    /// dedicated registration trait: the type supplies its own id via
    /// [`id`](Self::id).
    fn path_root() -> Path<Self, Self>
    where
        Self: Sized,
    {
        Path::from_model_id(Self::id())
    }

    /// A [`Path`] from this embedded type to the field at `index`.
    fn path_field<U>(index: usize) -> Path<Self, U>
    where
        Self: Sized,
    {
        Path::field_at(Self::id(), index)
    }
}
