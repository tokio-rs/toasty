use super::{Deferred, Load, Model, Register, RelationManyField};

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldId, FieldTy, ModelId};
use toasty_core::stmt;

impl<M: Model> RelationManyField for Vec<M> {
    type Model = M;

    const DEFERRED: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn many_relation_field_ty(
        singular: Name,
        pair: Option<FieldId>,
        via: Option<stmt::Path>,
    ) -> FieldTy {
        many_relation_field_ty::<M>(singular, pair, via)
    }
}

impl<M: Model> RelationManyField for Deferred<Vec<M>> {
    type Model = M;

    const DEFERRED: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn many_relation_field_ty(
        singular: Name,
        pair: Option<FieldId>,
        via: Option<stmt::Path>,
    ) -> FieldTy {
        many_relation_field_ty::<M>(singular, pair, via)
    }
}

fn many_relation_field_ty<M: Model>(
    singular: Name,
    pair: Option<FieldId>,
    via: Option<stmt::Path>,
) -> FieldTy {
    let target = <M as Register>::id();
    let expr_ty = stmt::Type::List(Box::new(stmt::Type::Model(target)));
    let cardinality = app::Cardinality::Many { singular };

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
