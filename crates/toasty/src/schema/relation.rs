use super::{Deferred, Field, Load, Model};

use toasty_core::schema::Name;
use toasty_core::schema::app::{FieldId, FieldTy, ForeignKey};
use toasty_core::stmt;

/// Marker for a direct relation scope.
///
/// Direct relation scopes can create new records because Toasty can populate
/// the target foreign key from the source record.
pub enum Direct {}

/// Marker for a multi-step relation scope.
///
/// Via relation scopes are queryable, but do not expose relation-scoped
/// creation because creating the target would require materializing one or
/// more intermediate records.
pub enum Via {}

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

    /// The "one-side" accessor type produced by the field accessor. Resolves
    /// to [`Model::One`] for non-nullable impls and [`Model::OptionOne`] for
    /// nullable impls.
    type One;

    /// The multi-step "one-side" accessor type produced by `via` relation
    /// accessors. Resolves to [`Model::ViaOne`] for non-nullable impls and
    /// [`Model::ViaOptionOne`] for nullable impls.
    type ViaOne;

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

/// A Rust field type that represents a `#[has_many(via = ...)]` field.
#[doc(hidden)]
pub trait ViaManyField: Load<Output = Self> {
    /// The typed target that the declared `via` path must produce.
    type PathTarget;

    /// The field accessor returned by generated `fields()` methods.
    type Path<Origin>;

    /// The runtime scope returned by model-instance relation methods.
    type Scope;

    /// The runtime query returned by query relation methods.
    type Query;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Build the accessor path from a raw path.
    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin>;

    /// Build a relation scope from an associated source query and path.
    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope;

    /// Build a relation query from an associated source query and path.
    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query;
}

/// A Rust field type that represents a `#[has_one(via = ...)]` field.
#[doc(hidden)]
pub trait ViaOneField: Load<Output = Self> {
    /// The typed target that the declared `via` path must produce from a single
    /// source model.
    type PathTarget;

    /// The typed target that this field produces from a list source.
    type ManyPathTarget;

    /// The field accessor returned by generated `fields()` methods.
    type Path<Origin>;

    /// The field accessor returned from list-context `fields()` methods.
    type ManyPath<Origin>;

    /// The runtime scope returned by model-instance relation methods.
    type Scope;

    /// The runtime query returned by query relation methods.
    type Query;

    /// Whether the field stores its value in a deferred load slot.
    const DEFERRED: bool;

    /// Whether the field is nullable.
    const NULLABLE: bool;

    /// Reloads this relation field from a returned value.
    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()>;

    /// Build the single-source accessor path from a raw path.
    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin>;

    /// Build the list-source accessor path from a raw path.
    fn new_many_path<Origin>(
        path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
    ) -> Self::ManyPath<Origin>;

    /// Build a relation scope from an associated source query and path.
    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope;

    /// Build a relation query from an associated source query and path.
    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query;
}

impl<M: Model> ViaManyField for Vec<M> {
    type PathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaMany;
    type Query = M::ViaMany;

    const DEFERRED: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_many(crate::stmt::Association::many(source, path))
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many(source, path))
    }
}

impl<M: Model> ViaManyField for Deferred<Vec<M>> {
    type PathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaMany;
    type Query = M::ViaMany;

    const DEFERRED: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_many(crate::stmt::Association::many(source, path))
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many(source, path))
    }
}

impl<M: Model> ViaOneField for M {
    type PathTarget = M;
    type ManyPathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::Path<Origin>;
    type ManyPath<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaOne;
    type Query = M::ViaMany;

    const DEFERRED: bool = false;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_path(path)
    }

    fn new_many_path<Origin>(
        path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
    ) -> Self::ManyPath<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_one(
            crate::stmt::IntoStatement::into_statement(crate::stmt::Association::one(source, path))
                .into_query()
                .unwrap(),
        )
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many_via_one(source, path))
    }
}

impl<M: Model> ViaOneField for Option<M> {
    type PathTarget = M;
    type ManyPathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::Path<Origin>;
    type ManyPath<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaOptionOne;
    type Query = M::ViaMany;

    const DEFERRED: bool = false;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_path(path)
    }

    fn new_many_path<Origin>(
        path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
    ) -> Self::ManyPath<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_option_one(
            crate::stmt::IntoStatement::into_statement(crate::stmt::Association::one(source, path))
                .into_query()
                .unwrap(),
        )
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many_via_one(source, path))
    }
}

impl<M: Model> ViaOneField for Deferred<M> {
    type PathTarget = M;
    type ManyPathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::Path<Origin>;
    type ManyPath<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaOne;
    type Query = M::ViaMany;

    const DEFERRED: bool = true;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_path(path)
    }

    fn new_many_path<Origin>(
        path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
    ) -> Self::ManyPath<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_one(
            crate::stmt::IntoStatement::into_statement(crate::stmt::Association::one(source, path))
                .into_query()
                .unwrap(),
        )
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many_via_one(source, path))
    }
}

impl<M: Model> ViaOneField for Deferred<Option<M>> {
    type PathTarget = M;
    type ManyPathTarget = crate::stmt::List<M>;
    type Path<Origin> = M::Path<Origin>;
    type ManyPath<Origin> = M::ManyField<Origin>;
    type Scope = M::ViaOptionOne;
    type Query = M::ViaMany;

    const DEFERRED: bool = true;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn new_path<Origin>(path: crate::stmt::Path<Origin, Self::PathTarget>) -> Self::Path<Origin> {
        M::new_path(path)
    }

    fn new_many_path<Origin>(
        path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
    ) -> Self::ManyPath<Origin> {
        M::new_many_field(path)
    }

    fn scope_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Scope {
        M::new_via_option_one(
            crate::stmt::IntoStatement::into_statement(crate::stmt::Association::one(source, path))
                .into_query()
                .unwrap(),
        )
    }

    fn query_from_association<Source: Model>(
        source: crate::stmt::Query<crate::stmt::List<Source>>,
        path: crate::stmt::Path<Source, Self::PathTarget>,
    ) -> Self::Query {
        M::new_via_many(crate::stmt::Association::many_via_one(source, path))
    }
}

macro_rules! impl_projected_via_field {
    ($ty:ty) => {
        impl ViaOneField for $ty {
            type PathTarget = <$ty as Field>::ExprTarget;
            type ManyPathTarget = crate::stmt::List<<$ty as Field>::ExprTarget>;
            type Path<Origin> = <$ty as Field>::Path<Origin>;
            type ManyPath<Origin> = <$ty as Field>::ListPath<Origin>;
            type Scope = crate::stmt::ProjectedOne<$ty>;
            type Query = crate::stmt::ProjectedMany<$ty>;

            const DEFERRED: bool = <$ty as Field>::DEFERRED;
            const NULLABLE: bool = <$ty as Field>::NULLABLE;

            fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
                <Self as Load>::reload(target, value)
            }

            fn new_path<Origin>(
                path: crate::stmt::Path<Origin, Self::PathTarget>,
            ) -> Self::Path<Origin> {
                <$ty as Field>::new_path(path)
            }

            fn new_many_path<Origin>(
                path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
            ) -> Self::ManyPath<Origin> {
                <$ty as Field>::new_list_path(path)
            }

            fn scope_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Scope {
                crate::stmt::ProjectedOne::from_association(source, path)
            }

            fn query_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Query {
                crate::stmt::ProjectedMany::from_association(source, path)
            }
        }

        impl ViaOneField for Deferred<$ty> {
            type PathTarget = <$ty as Field>::ExprTarget;
            type ManyPathTarget = crate::stmt::List<<$ty as Field>::ExprTarget>;
            type Path<Origin> = <$ty as Field>::Path<Origin>;
            type ManyPath<Origin> = <$ty as Field>::ListPath<Origin>;
            type Scope = crate::stmt::ProjectedOne<$ty>;
            type Query = crate::stmt::ProjectedMany<$ty>;

            const DEFERRED: bool = true;
            const NULLABLE: bool = <$ty as Field>::NULLABLE;

            fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
                target.unload();
                Ok(())
            }

            fn new_path<Origin>(
                path: crate::stmt::Path<Origin, Self::PathTarget>,
            ) -> Self::Path<Origin> {
                <$ty as Field>::new_path(path)
            }

            fn new_many_path<Origin>(
                path: crate::stmt::Path<Origin, Self::ManyPathTarget>,
            ) -> Self::ManyPath<Origin> {
                <$ty as Field>::new_list_path(path)
            }

            fn scope_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Scope {
                crate::stmt::ProjectedOne::from_association(source, path)
            }

            fn query_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Query {
                crate::stmt::ProjectedMany::from_association(source, path)
            }
        }

        impl ViaManyField for Vec<$ty> {
            type PathTarget = crate::stmt::List<<$ty as Field>::ExprTarget>;
            type Path<Origin> = <$ty as Field>::ListPath<Origin>;
            type Scope = crate::stmt::ProjectedMany<$ty>;
            type Query = crate::stmt::ProjectedMany<$ty>;

            const DEFERRED: bool = false;

            fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
                <Self as Load>::reload(target, value)
            }

            fn new_path<Origin>(
                path: crate::stmt::Path<Origin, Self::PathTarget>,
            ) -> Self::Path<Origin> {
                <$ty as Field>::new_list_path(path)
            }

            fn scope_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Scope {
                crate::stmt::ProjectedMany::from_association(source, path)
            }

            fn query_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Query {
                crate::stmt::ProjectedMany::from_association(source, path)
            }
        }

        impl ViaManyField for Deferred<Vec<$ty>> {
            type PathTarget = crate::stmt::List<<$ty as Field>::ExprTarget>;
            type Path<Origin> = <$ty as Field>::ListPath<Origin>;
            type Scope = crate::stmt::ProjectedMany<$ty>;
            type Query = crate::stmt::ProjectedMany<$ty>;

            const DEFERRED: bool = true;

            fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
                target.unload();
                Ok(())
            }

            fn new_path<Origin>(
                path: crate::stmt::Path<Origin, Self::PathTarget>,
            ) -> Self::Path<Origin> {
                <$ty as Field>::new_list_path(path)
            }

            fn scope_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Scope {
                crate::stmt::ProjectedMany::from_association(source, path)
            }

            fn query_from_association<Source: Model>(
                source: crate::stmt::Query<crate::stmt::List<Source>>,
                path: crate::stmt::Path<Source, Self::PathTarget>,
            ) -> Self::Query {
                crate::stmt::ProjectedMany::from_association(source, path)
            }
        }
    };
}

macro_rules! impl_projected_via_fields {
    ($($ty:ty),* $(,)?) => {
        $(
            impl_projected_via_field!($ty);
            impl_projected_via_field!(Option<$ty>);
        )*
    };
}

impl_projected_via_fields!(
    String,
    uuid::Uuid,
    bool,
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    u64,
    isize,
    usize,
    f32,
    f64,
    Vec<u8>,
);

#[cfg(feature = "rust_decimal")]
impl_projected_via_fields!(rust_decimal::Decimal);

#[cfg(feature = "bigdecimal")]
impl_projected_via_fields!(bigdecimal::BigDecimal);
