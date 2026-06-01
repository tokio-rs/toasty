use super::{Deferred, Load, Model, QueryMany, QueryOne, QueryOptionOne};

use toasty_core::schema::app::ModelId;
use toasty_core::schema::app::{self, FieldId, FieldTy, ForeignKey};
use toasty_core::stmt;

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
    ///
    /// Relation accessors build the list query (from a filter or by wrapping a
    /// singular association via [`Model::wrap_query`]) and pass it here; the
    /// association's path is preserved through the wrap so generated mutators
    /// (insert, remove, create) can read it.
    fn make_one(query: QueryMany<Self::Model>) -> Self::One;

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

impl<M: Model> RelationOneField for M {
    type Model = M;
    type One = QueryOne<M>;
    type Expr = M;

    const DEFERRED: bool = false;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn make_one(query: QueryMany<Self::Model>) -> Self::One {
        <M as Model>::query_one(query)
    }

    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_relation_field_ty::<M>(pair, via)
    }

    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_relation_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> RelationOneField for Option<M> {
    type Model = M;
    type One = QueryOptionOne<M>;
    type Expr = Option<M>;

    const DEFERRED: bool = false;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn make_one(query: QueryMany<Self::Model>) -> Self::One {
        <M as Model>::query_first(query)
    }

    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_relation_field_ty::<M>(pair, via)
    }

    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_relation_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> RelationOneField for Deferred<M> {
    type Model = M;
    type One = QueryOne<M>;
    type Expr = M;

    const DEFERRED: bool = true;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn make_one(query: QueryMany<Self::Model>) -> Self::One {
        <M as Model>::query_one(query)
    }

    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_relation_field_ty::<M>(pair, via)
    }

    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_relation_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> RelationOneField for Deferred<Option<M>> {
    type Model = M;
    type One = QueryOptionOne<M>;
    type Expr = Option<M>;

    const DEFERRED: bool = true;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn make_one(query: QueryMany<Self::Model>) -> Self::One {
        <M as Model>::query_first(query)
    }

    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_relation_field_ty::<M>(pair, via)
    }

    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_relation_field_ty::<M>(foreign_key)
    }
}

fn has_one_relation_field_ty<M: Model>(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
    let target = <M as Model>::id();
    let expr_ty = stmt::Type::Model(target);
    let cardinality = app::Cardinality::One;

    match via {
        Some(path) => FieldTy::Via(app::Via::new(target, expr_ty, cardinality, path, None)),
        None => FieldTy::Has(app::Has {
            target,
            expr_ty,
            cardinality,
            pair_id: pair.unwrap_or(FieldId {
                model: ModelId(usize::MAX),
                index: usize::MAX,
            }),
        }),
    }
}

fn belongs_to_relation_field_ty<M: Model>(foreign_key: ForeignKey) -> FieldTy {
    let target = <M as Model>::id();

    FieldTy::BelongsTo(app::BelongsTo {
        target,
        expr_ty: stmt::Type::Model(target),
        // The pair is populated at runtime.
        pair: None,
        foreign_key,
    })
}
