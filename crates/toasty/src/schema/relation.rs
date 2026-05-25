use super::{Load, Model};
use crate::stmt::{IntoExpr, IntoInsert, List, Path};

use toasty_core::schema::Name;
use toasty_core::schema::app::{FieldId, FieldTy, ForeignKey};
use toasty_core::stmt;

/// Describes how a model participates in associations.
///
/// This trait is implemented by `#[derive(Model)]` and provides the associated
/// types that generated relation code uses to construct query builders, create
/// builders, and field accessors for the relation target.
///
/// Users do not implement this trait manually.
pub trait Relation: Load<Output = Self> {
    /// The target model
    type Model: Model;

    /// The target expression (e.g. `Option<Model>`)
    type Expr;

    /// The query builder type for querying this relation's target.
    type Query;

    /// Create builder type for this relation's target model
    type Create: Default + IntoInsert<Model = Self::Model> + IntoExpr<Self::Model>;

    /// HasMany relation type
    type Many;

    /// The field accessor type used when this model appears as the "many" side
    /// of a has-many relation, parameterized by the origin model.
    type ManyField<Origin>;

    /// The has-one relation wrapper type for this model.
    type One;

    /// The field accessor type used when this model appears as the "one" side
    /// of a has-one relation, parameterized by the origin model.
    type OneField<Origin>;

    /// The optional has-one relation wrapper type. Used when the foreign key
    /// is nullable, making the association optional.
    type OptionOne;

    /// Return a fresh, default-initialized create builder.
    fn new_create() -> Self::Create {
        Self::Create::default()
    }

    /// Construct a `ManyField` from a path targeting a list of the model.
    fn new_many_field<Origin>(path: Path<Origin, List<Self::Model>>) -> Self::ManyField<Origin>;

    /// Map a field name string to its [`FieldId`].
    ///
    /// Panics if `name` does not match any field on the model.
    fn field_name_to_id(name: &str) -> FieldId;

    /// Returns `true` if this relation target is nullable (i.e., wrapped in
    /// `Option`). The default is `false`.
    fn nullable() -> bool {
        false
    }
}

/// A Rust field type that represents a `#[has_many]` relation.
///
/// This is implemented by [`Deferred<Vec<T>>`](super::Deferred) for lazy
/// relations and by `Vec<T>` for eager relations. The target model/query-builder
/// metadata stays on [`Relation`]; this trait only describes how the field
/// itself contributes relation schema metadata.
pub trait HasManyField: Load<Output = Self> {
    /// The relation target type carried by this field.
    type Target: Relation;

    /// Returns `true` if this relation field is nullable.
    fn nullable() -> bool {
        <Self::Target as Relation>::nullable()
    }

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

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
    fn has_many_field_ty(singular: Name, pair: Option<FieldId>, via: Option<stmt::Path>)
    -> FieldTy;
}

/// A Rust field type that represents a `#[has_one]` relation.
///
/// This is implemented by [`Deferred<T>`](super::Deferred) for lazy relations
/// and by `T` for eager relations. `T` may be `Option<Model>` for nullable
/// `has_one` fields.
pub trait HasOneField: Load<Output = Self> {
    /// The relation target type carried by this field.
    type Target: Relation;

    /// Returns `true` if this relation field is nullable.
    fn nullable() -> bool {
        <Self::Target as Relation>::nullable()
    }

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Build the [`FieldTy`] for a `HasOne` relation field, given an
    /// optional paired `BelongsTo` field on the target model resolved
    /// from `#[has_one(pair = <field>)]`. When `None`, the linker selects
    /// the pair by searching the target for a unique `BelongsTo` back to
    /// the source.
    ///
    /// `via` carries the fully resolved [`stmt::Path`] of a
    /// `#[has_one(via = a.b)]` multi-step relation, rooted at the declaring
    /// model. A `via` relation has no pair.
    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy;
}

/// A Rust field type that represents a `#[belongs_to]` relation.
///
/// This is implemented by [`Deferred<T>`](super::Deferred) for lazy relations
/// and by `T` for eager relations. `T` may be `Option<Model>` for nullable
/// `belongs_to` fields.
pub trait BelongsToField: Load<Output = Self> {
    /// The relation target type carried by this field.
    type Target: Relation;

    /// Returns `true` if this relation field is nullable.
    fn nullable() -> bool {
        <Self::Target as Relation>::nullable()
    }

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Build the [`FieldTy`] for a `BelongsTo` relation field, given the
    /// foreign key resolved from the field's `#[belongs_to(...)]` attribute.
    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy;
}
