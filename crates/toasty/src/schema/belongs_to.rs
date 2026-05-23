use super::{BelongsToField, Deferred, Register, Relation};

use toasty_core::schema::app::{self, FieldTy, ForeignKey};
use toasty_core::stmt;

impl<T: Relation> BelongsToField for Deferred<T> {
    type Target = T;

    fn nullable() -> bool {
        <T as Relation>::nullable()
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
