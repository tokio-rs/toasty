use super::{Deferred, HasManyField, Load, Register, Relation};

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldId, FieldTy, ModelId};
use toasty_core::stmt;

impl<T: Relation> HasManyField for Deferred<Vec<T>> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn has_many_field_ty(
        singular: Name,
        pair: Option<FieldId>,
        via: Option<stmt::Path>,
    ) -> FieldTy {
        has_many_field_ty::<T>(singular, pair, via)
    }
}

impl<T: Relation> HasManyField for Vec<T> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn has_many_field_ty(
        singular: Name,
        pair: Option<FieldId>,
        via: Option<stmt::Path>,
    ) -> FieldTy {
        has_many_field_ty::<T>(singular, pair, via)
    }
}

fn has_many_field_ty<T: Relation>(
    singular: Name,
    pair: Option<FieldId>,
    via: Option<stmt::Path>,
) -> FieldTy {
    let target = <T::Model as Register>::id();
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
