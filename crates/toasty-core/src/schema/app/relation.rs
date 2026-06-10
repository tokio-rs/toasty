//! Relation types that connect models to each other.
//!
//! Toasty supports five kinds of relations:
//!
//! - [`BelongsTo`] -- the owning side; stores the foreign key fields.
//! - [`Has`] -- the inverse side for one-to-many and one-to-one relationships
//!   paired with [`BelongsTo`].
//! - [`HasItems`] -- the parent-side counterpart to [`ItemParent`]; lowers to
//!   a partition-scoped query with a sort-key prefix filter rather than a
//!   foreign-key join.
//! - [`ItemParent`] -- a child's reference to its item-collection parent;
//!   carries no foreign-key columns (the symmetric primary key encodes the
//!   parent directly).
//! - [`Via`] -- a multi-step relation that walks a chain of existing
//!   relations.

mod belongs_to;
pub use belongs_to::BelongsTo;

mod has;
pub use has::{Cardinality, Has};

mod has_items;
pub use has_items::HasItems;

mod item_parent;
pub use item_parent::ItemParent;

mod via;
pub use via::Via;
