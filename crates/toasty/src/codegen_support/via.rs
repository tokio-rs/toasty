//! Helpers for the `#[has_many(via = …)]` code the derive macro emits.

use crate::schema::Model;

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldTy, Via};
use toasty_core::stmt;

/// Build the [`FieldTy::Via`] for a relation-terminal `#[has_many(via = …)]`
/// field reaching model `M`. The per-model
/// [`ViaManyField`](crate::schema::ViaManyField) impl the derive emits
/// delegates here so the construction (and `Box`/type plumbing) stays in this
/// crate.
pub fn model_via_field_ty<M: Model>(singular: Name, path: stmt::Path) -> FieldTy {
    let target = <M as Model>::id();
    let expr_ty = stmt::Type::List(Box::new(stmt::Type::Model(target)));
    FieldTy::Via(Via::new(
        target,
        expr_ty,
        app::Cardinality::Many { singular },
        path,
        None,
    ))
}
