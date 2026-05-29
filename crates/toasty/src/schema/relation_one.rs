use super::{Deferred, Load, Model, Register, RelationOneField};

use toasty_core::schema::app::ModelId;
use toasty_core::schema::app::{self, FieldId, FieldTy, ForeignKey};
use toasty_core::stmt;

impl<M: Model> RelationOneField for M {
    type Model = M;
    type One = <M as Model>::QueryOne;
    type Expr = M;

    const DEFERRED: bool = false;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn make_one(query: <Self::Model as Model>::Query) -> Self::One {
        <M as Model>::query_one(query)
    }

    fn make_one_from_assoc(assoc: crate::stmt::Association<Self::Model>) -> Self::One {
        <M as Model>::query_from_assoc_one(assoc)
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
    type One = <M as Model>::QueryOptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = false;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn make_one(query: <Self::Model as Model>::Query) -> Self::One {
        <M as Model>::query_first(query)
    }

    fn make_one_from_assoc(assoc: crate::stmt::Association<Self::Model>) -> Self::One {
        <M as Model>::query_from_assoc_first(assoc)
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
    type One = <M as Model>::QueryOne;
    type Expr = M;

    const DEFERRED: bool = true;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn make_one(query: <Self::Model as Model>::Query) -> Self::One {
        <M as Model>::query_one(query)
    }

    fn make_one_from_assoc(assoc: crate::stmt::Association<Self::Model>) -> Self::One {
        <M as Model>::query_from_assoc_one(assoc)
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
    type One = <M as Model>::QueryOptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = true;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn make_one(query: <Self::Model as Model>::Query) -> Self::One {
        <M as Model>::query_first(query)
    }

    fn make_one_from_assoc(assoc: crate::stmt::Association<Self::Model>) -> Self::One {
        <M as Model>::query_from_assoc_first(assoc)
    }

    fn has_one_relation_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_relation_field_ty::<M>(pair, via)
    }

    fn belongs_to_relation_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_relation_field_ty::<M>(foreign_key)
    }
}

fn has_one_relation_field_ty<M: Model>(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
    let target = <M as Register>::id();
    let expr_ty = stmt::Type::Model(target);
    let cardinality = app::Cardinality::One;

    match via {
        Some(path) => FieldTy::Via(app::Via::new(target, expr_ty, cardinality, path)),
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
    let target = <M as Register>::id();

    FieldTy::BelongsTo(app::BelongsTo {
        target,
        expr_ty: stmt::Type::Model(target),
        // The pair is populated at runtime.
        pair: None,
        foreign_key,
    })
}
