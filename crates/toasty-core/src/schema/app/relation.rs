//! Relation types that connect models to each other.
//!
//! Toasty supports three kinds of relations:
//!
//! - [`BelongsTo`] -- the owning side; stores the foreign key fields.
//! - [`HasMany`] -- the inverse side for a one-to-many relationship.
//! - [`HasOne`] -- the inverse side for a one-to-one relationship.

mod belongs_to;
pub use belongs_to::BelongsTo;

mod has_many;
pub use has_many::HasMany;

mod has_one;
pub use has_one::HasOne;
