use super::has_many::has_kind;
use super::{Deferred, HasOneField, Load, Register, Relation};

use toasty_core::schema::app::{self, FieldId, FieldTy};
use toasty_core::stmt;

impl<T: Relation> HasOneField for Deferred<T> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<T>(pair, via)
    }
}

impl<T: Relation> HasOneField for T {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<T>(pair, via)
    }
}

fn has_one_field_ty<T: Relation>(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
    FieldTy::Has(app::Has {
        target: <T::Model as Register>::id(),
        expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
        cardinality: app::HasCardinality::One,
        kind: has_kind(pair, via),
    })
}
