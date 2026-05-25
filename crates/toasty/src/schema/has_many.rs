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
    FieldTy::Has(app::Has {
        target: <T::Model as Register>::id(),
        expr_ty: stmt::Type::List(Box::new(stmt::Type::Model(<T::Model as Register>::id()))),
        cardinality: app::HasCardinality::Many { singular },
        kind: has_kind(pair, via),
    })
}

/// Build a [`HasKind`](app::HasKind) from the macro-supplied `pair` / `via`
/// attributes. `via` declares a multi-step relation and carries the fully
/// resolved [`stmt::Path`] emitted by the derive; otherwise the relation is
/// direct, and a direct relation with no explicit `pair` gets a placeholder
/// that the schema linker resolves.
pub(super) fn has_kind(pair: Option<FieldId>, via: Option<stmt::Path>) -> app::HasKind {
    match via {
        Some(path) => app::HasKind::Via(app::Via::new(path)),
        None => app::HasKind::Direct(pair.unwrap_or(FieldId {
            model: ModelId(usize::MAX),
            index: usize::MAX,
        })),
    }
}
