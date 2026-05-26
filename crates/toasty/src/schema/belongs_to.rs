use super::{BelongsToField, Deferred, Load, Model, Register};

use toasty_core::schema::app::{self, FieldTy, ForeignKey};
use toasty_core::stmt;

impl<M: Model> BelongsToField for M {
    type Model = M;
    type One = M::One;
    type Expr = M;

    const DEFERRED: bool = false;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> BelongsToField for Option<M> {
    type Model = M;
    type One = M::OptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = false;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, value: stmt::Value) -> crate::Result<()> {
        <Self as Load>::reload(target, value)
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> BelongsToField for Deferred<M> {
    type Model = M;
    type One = M::One;
    type Expr = M;

    const DEFERRED: bool = true;
    const NULLABLE: bool = false;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<M>(foreign_key)
    }
}

impl<M: Model> BelongsToField for Deferred<Option<M>> {
    type Model = M;
    type One = M::OptionOne;
    type Expr = Option<M>;

    const DEFERRED: bool = true;
    const NULLABLE: bool = true;

    fn reload(target: &mut Self, _value: stmt::Value) -> crate::Result<()> {
        target.unload();
        Ok(())
    }

    fn belongs_to_field_ty(foreign_key: ForeignKey) -> FieldTy {
        belongs_to_field_ty::<M>(foreign_key)
    }
}

fn belongs_to_field_ty<M: Model>(foreign_key: ForeignKey) -> FieldTy {
    FieldTy::BelongsTo(app::BelongsTo {
        target: <M as Register>::id(),
        expr_ty: stmt::Type::Model(<M as Register>::id()),
        // The pair is populated at runtime.
        pair: None,
        foreign_key,
    })
}
