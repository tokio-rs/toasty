use super::{Load, Model};

use toasty_core::schema::Name;
use toasty_core::schema::app::{FieldId, FieldTy};
use toasty_core::stmt;

/// A Rust field type that represents a `#[has_many]` relation.
///
/// Implemented by [`Vec<M>`](Vec) (eager) and
/// [`Deferred<Vec<M>>`](super::Deferred) (lazy) where `M: Model`. The set of
/// impls is the source of truth for which Rust shapes are valid as a
/// has-many field: anything outside those two combinations does not satisfy
/// the trait.
pub trait RelationManyField: Load<Output = Self> {
    /// The target model that this field references.
    type Target: Model;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// A has-many is a collection; the collection itself is always present
    /// even when empty, so a has-many field is never nullable.
    const NULLABLE: bool = false;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Build the [`FieldTy`] for a `HasMany` relation field, given the
    /// singular name derived from the field identifier and an optional
    /// paired `BelongsTo` field on the target model resolved from
    /// `#[has_many(pair = <field>)]`. When `None`, the linker selects the
    /// pair by searching the target for a unique `BelongsTo` back to the
    /// source.
    ///
    /// `via` carries the fully resolved [`stmt::Path`] of a
    /// `#[has_many(via = a.b)]` multi-step relation, rooted at the declaring
    /// model. A `via` relation has no pair.
    fn many_relation_field_ty(
        singular: Name,
        pair: Option<FieldId>,
        via: Option<stmt::Path>,
    ) -> FieldTy;
}
