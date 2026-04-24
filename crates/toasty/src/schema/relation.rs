use super::{Load, Model};
use crate::stmt::{IntoExpr, IntoInsert, List, Path};

use toasty_core::schema::Name;
use toasty_core::schema::app::{FieldId, FieldTy, ForeignKey};

/// Describes how a model participates in associations.
///
/// This trait is implemented by `#[derive(Model)]` and provides the associated
/// types that the generated `HasMany`, `HasOne`, and `BelongsTo` wrappers use
/// to construct query builders, create builders, and field accessors for
/// the relation target.
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

    /// Build the [`FieldTy`] for a `BelongsTo` relation wrapper, given the
    /// foreign key resolved from the field's `#[belongs_to(...)]` attribute.
    ///
    /// Only [`BelongsTo`](super::BelongsTo) overrides this; the default
    /// panics so that misuse (e.g. applying `#[belongs_to]` to a field whose
    /// type is not a `BelongsTo<T>`) fails loudly.
    fn belongs_to_field_ty(_foreign_key: ForeignKey) -> FieldTy {
        unimplemented!("not a BelongsTo relation wrapper")
    }

    /// Build the [`FieldTy`] for a `HasMany` relation wrapper, given the
    /// singular name derived from the field identifier and an optional
    /// paired `BelongsTo` field on the target model resolved from
    /// `#[has_many(pair = <field>)]`. When `None`, the linker selects the
    /// pair by searching the target for a unique `BelongsTo` back to the
    /// source.
    ///
    /// Only [`HasMany`](super::HasMany) overrides this.
    fn has_many_field_ty(_singular: Name, _pair: Option<FieldId>) -> FieldTy {
        unimplemented!("not a HasMany relation wrapper")
    }

    /// Build the [`FieldTy`] for a `HasOne` relation wrapper, given an
    /// optional paired `BelongsTo` field on the target model resolved
    /// from `#[has_one(pair = <field>)]`. When `None`, the linker selects
    /// the pair by searching the target for a unique `BelongsTo` back to
    /// the source.
    ///
    /// Only [`HasOne`](super::HasOne) overrides this.
    fn has_one_field_ty(_pair: Option<FieldId>) -> FieldTy {
        unimplemented!("not a HasOne relation wrapper")
    }
}
