use super::{Deferred, HasOneField, Load, Model, Register};

use toasty_core::schema::app::ModelId;
use toasty_core::schema::app::{self, FieldId, FieldTy};
use toasty_core::stmt;

impl<M: Model> HasOneField for M {
    type Model = M;
    type One = M::One;
    type Expr = M;

    const DEFERRED: bool = false;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<M>(pair, via)
    }
}

impl<M: Model> HasOneField for Option<M> {
    type Model = M;
    type One = M::OptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = false;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<M>(pair, via)
    }
}

impl<M: Model> HasOneField for Deferred<M> {
    type Model = M;
    type One = M::One;
    type Expr = M;

    const DEFERRED: bool = true;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<M>(pair, via)
    }
}

impl<M: Model> HasOneField for Deferred<Option<M>> {
    type Model = M;
    type One = M::OptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = true;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<M>(pair, via)
    }
}

fn has_one_field_ty<M: Model>(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
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
