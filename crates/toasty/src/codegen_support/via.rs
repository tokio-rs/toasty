//! Helpers for the `#[has_one(via = …)]` and `#[has_many(via = …)]` code the
//! derive macro emits.

use crate::schema::{Model, RelationOneField};
use crate::stmt::Path;

use toasty_core::schema::Name;
use toasty_core::schema::app::{self, FieldTy, Via};
use toasty_core::stmt;

/// A declared singular via's expression type that permits a step targeting
/// `Step`. Required vias permit only required steps; optional vias permit both.
#[diagnostic::on_unimplemented(
    message = "`#[has_one(via = ...)]` result `{Self}` cannot traverse a step targeting `{Step}`",
    label = "incompatible via step",
    note = "a required via cannot traverse optional relations; declare the via field as `Option<_>` or `Deferred<Option<_>>`"
)]
pub trait AllowsViaStep<Step> {}

// Report the via compatibility error instead of a missing `Model` impl for
// an optional step.
#[diagnostic::do_not_recommend]
impl<Declared: Model, Step: Model> AllowsViaStep<Step> for Declared {}
#[diagnostic::do_not_recommend]
impl<Declared: Model, Step: Model> AllowsViaStep<Step> for Option<Declared> {}
#[diagnostic::do_not_recommend]
impl<Declared: Model, Step: Model> AllowsViaStep<Option<Step>> for Option<Declared> {}

/// Check a singular via step before navigating to the next field, which may
/// have different optionality. Borrowing the handle leaves it available for
/// the next accessor call.
pub fn check_one_step<F, Origin, Target>(_: &impl Into<Path<Origin, Target>>)
where
    F: RelationOneField,
    F::Expr: AllowsViaStep<Target>,
    Target: RelationOneField,
{
}

/// Validate that a singular via reaches the declared model and convert its
/// typed path to a schema path. Each step's optionality is checked separately
/// by [`check_one_step`].
pub fn into_one_path<F, Origin, Target>(path: Path<Origin, Target>) -> stmt::Path
where
    F: RelationOneField,
    Target: RelationOneField<Target = F::Target>,
{
    path.into()
}

/// Build the [`FieldTy::Via`] for a relation-terminal `#[has_many(via = …)]`
/// field reaching model `M`. The per-model
/// [`ViaTarget`](crate::schema::ViaTarget) impl the derive emits
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
