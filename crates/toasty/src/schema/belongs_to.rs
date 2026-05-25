use super::{BelongsToField, Deferred, Load, Register, Relation};

use toasty_core::schema::app::{self, FieldTy, ForeignKey};
use toasty_core::stmt;

impl<T: Relation> BelongsToField for Deferred<T> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<T>(foreign_key)
    }
}

impl<T: Relation> BelongsToField for T {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
    }

    const DEFERRED: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<T>(foreign_key)
    }
}

fn belongs_to_field_ty<T: Relation>(foreign_key: ForeignKey) -> FieldTy {
    FieldTy::BelongsTo(app::BelongsTo {
        target: <T::Model as Register>::id(),
        expr_ty: stmt::Type::Model(<T::Model as Register>::id()),
        // The pair is populated at runtime.
        pair: None,
        foreign_key,
    })
}
