use super::has_many::has_kind;
use super::{Deferred, HasOneField, Register, Relation};

use toasty_core::schema::app::{self, FieldId, FieldTy};
use toasty_core::stmt;

impl<T: Relation> HasOneField for Deferred<T> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    fn has_one_field_ty(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
        has_one_field_ty::<T>(pair, via)
    }
}

fn has_one_field_ty<T: Relation>(pair: Option<FieldId>, via: Option<stmt::Path>) -> FieldTy {
    FieldTy::HasOne(app::HasOne {
        target: <T::Model as Register>::id(),
        expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
        kind: has_kind(pair, via),
    })
}
