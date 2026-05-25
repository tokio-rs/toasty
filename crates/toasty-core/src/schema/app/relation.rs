//! Relation types that connect models to each other.
//!
//! Toasty supports three kinds of relations:
//!
//! - [`BelongsTo`] -- the owning side; stores the foreign key fields.
//! - [`Has`] -- the inverse side for one-to-many and one-to-one relationships.

mod belongs_to;
pub use belongs_to::BelongsTo;

mod has;
pub use has::{Has, HasCardinality};

mod via;
pub use via::{HasKind, Via};
