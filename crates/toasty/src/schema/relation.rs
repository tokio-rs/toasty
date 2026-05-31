use super::{Load, Model, QueryMany};

use toasty_core::schema::Name;
use toasty_core::schema::app::{FieldId, FieldTy, ForeignKey};
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
    type Model: Model;

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

/// A Rust field type that represents a `#[has_one]` or `#[belongs_to]`
/// relation.
///
/// Implemented by `M`, `Option<M>`, `Deferred<M>`, and `Deferred<Option<M>>`
/// where `M: Model`. The `Option<...>` wrappers carry nullability; the
/// `Deferred<...>` wrappers carry deferred loading. Anything outside this
/// shape does not satisfy the trait.
pub trait RelationOneField: Load<Output = Self> {
    /// The target model that this field references.
    type Model: Model;

    /// The query type produced by the relation accessor. For non-nullable
    /// impls this is `<Model as Model>::Query<Model>`; for nullable impls it is
    /// `<Model as Model>::Query<Option<Model>>`.
    type One;

    /// The expression-level type used in create/update setters. Resolves to
    /// the unwrapped `Self::Model` for non-nullable impls and `Option<Self::Model>`
    /// for nullable impls.
    type Expr;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Whether the field is nullable (i.e. wrapped in `Option`).
    const NULLABLE: bool;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Narrow a list query targeting the related model into the appropriate
    /// "one" query — `Query<Model>` for non-nullable impls and
    /// `Query<Option<Model>>` for nullable impls.
    fn make_one(query: QueryMany<Self::Model>) -> Self::One;

    /// Build the appropriate "one" query from a singular association,
    /// preserving the association's path so generated mutators (insert,
    /// remove, create) can read it.
    fn make_one_from_assoc(assoc: crate::stmt::Association<Self::Model>) -> Self::One;

    /// Build the [`FieldTy`] for a `HasOne` relation field, given an
    /// optional paired `BelongsTo` field on the target model resolved
    /// from `#[has_one(pair = <field>)]`. When `None`, the linker selects
    /// the pair by searching the target for a unique `BelongsTo` back to
    /// the source.
    ///
    /// `via` carries the fully resolved [`stmt::Path`] of a
    /// `#[has_one(via = a.b)]` multi-step relation, rooted at the declaring
    /// model. A `via` relation has no pair.
    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy;

    /// Build the [`FieldTy`] for a `BelongsTo` relation field, given the
    /// foreign key resolved from the field's `#[belongs_to(...)]` attribute.
    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy;
}
