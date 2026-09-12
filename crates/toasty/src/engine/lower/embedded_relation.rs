//! Resolution of relation references reached through embedded types.
//!
//! A filter on a relation stored in an embedded type arrives as a path
//! expression into the embed's value — `Project(Ref(owner), [i])` for a
//! struct embed, `Project(Variant(Ref(owner), v), [i])` through a variant of
//! an enum embed. This module resolves such an expression to the
//! `belongs_to` it lands on (or passes through) and builds the expression
//! projecting the relation's key field(s) out of the host row, which is what
//! the [`lift_in_subquery`] rewrites substitute for the relation reference.
//!
//! The variant selection is part of the expression, so resolution needs no
//! context from the enclosing predicate: the steps after a
//! [`Expr::Variant`] index that variant's fields. The selection carries no
//! variant check of its own — the typed layer fixed the `is_variant` guards
//! next to the predicate when it was built (`Expr::with_variant_guards`),
//! and the key expression built here keeps the selection, so the guards
//! still apply to the substituted comparison.
//!
//! [`lift_in_subquery`]: super::lift_in_subquery

use toasty_core::{
    schema::app::{self, BelongsTo, FieldTy, VariantId},
    stmt::{self, Expr, ExprContext, ResolvedRef},
};

/// A `belongs_to` reached by projecting through embedded types.
pub(super) struct EmbeddedRelationRef<'a> {
    /// The relation definition.
    pub(super) belongs_to: &'a BelongsTo,

    /// Expression projecting the relation's key field(s) out of the host
    /// row: a single projection for a one-field foreign key, a record of
    /// projections otherwise.
    pub(super) key_expr: Expr,

    /// Path steps remaining after the relation field — a path into the
    /// relation's target model. Empty when the expression references the
    /// relation itself.
    pub(super) tail: Vec<Step>,
}

/// One step of an expression path: a field projection or a variant
/// selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Step {
    /// `Project(.., [index])`: a field of a record — a model, a struct embed,
    /// or a selected variant's payload.
    Field(usize),

    /// `Variant(.., variant)`: the payload of an enum value as `variant`.
    Variant(VariantId),
}

/// Peel nested `Project` and `Variant` layers, returning the base field
/// reference and the steps from it, outermost step first. Typed paths
/// produce nested layers (`Project(Variant(Project(Ref, [a]), v), [b])`),
/// so a single pattern-match is not enough.
pub(super) fn flatten_expr_path(expr: &Expr) -> Option<(stmt::ExprReference, Vec<Step>)> {
    match expr {
        Expr::Project(project) => {
            let (base, mut steps) = flatten_expr_path(&project.base)?;
            steps.extend(
                project
                    .projection
                    .as_slice()
                    .iter()
                    .map(|i| Step::Field(*i)),
            );
            Some((base, steps))
        }
        Expr::Variant(variant) => {
            let (base, mut steps) = flatten_expr_path(&variant.base)?;
            steps.push(Step::Variant(variant.variant));
            Some((base, steps))
        }
        Expr::Reference(reference @ stmt::ExprReference::Field { .. }) => {
            Some((*reference, vec![]))
        }
        _ => None,
    }
}

/// Build the expression applying `steps` to `base` — the inverse of
/// [`flatten_expr_path`]. Consecutive field steps merge into one projection.
pub(super) fn build_expr_path(base: Expr, steps: &[Step]) -> Expr {
    let mut expr = base;
    let mut fields: Vec<usize> = vec![];

    for step in steps {
        match step {
            Step::Field(index) => fields.push(*index),
            Step::Variant(variant) => {
                if !fields.is_empty() {
                    expr = Expr::project(expr, std::mem::take(&mut fields).as_slice());
                }
                expr = Expr::variant(expr, *variant);
            }
        }
    }

    if fields.is_empty() {
        expr
    } else {
        Expr::project(expr, fields.as_slice())
    }
}

/// Resolve `expr` — a path into an embedded type — to the `belongs_to` its
/// path lands on or passes through.
///
/// Returns `None` when the expression is not such a path: the base is not an
/// embedded field, a step lands on a non-relation leaf, a field step indexes
/// an enum without a variant selected, or a variant selection does not match
/// the enum it is applied to.
pub(super) fn resolve_embedded_relation<'a>(
    cx: &ExprContext<'a>,
    expr: &Expr,
) -> Option<EmbeddedRelationRef<'a>> {
    let (base, steps) = flatten_expr_path(expr)?;

    let ResolvedRef::Field(base_field) = cx.resolve_expr_reference(&base) else {
        return None;
    };
    let FieldTy::Embedded(embedded) = &base_field.ty else {
        return None;
    };

    let mut level = Level::embed(cx, embedded.target)?;
    // The expression reaching the current embed level.
    let mut level_expr = Expr::Reference(base);

    for (i, step) in steps.iter().enumerate() {
        match step {
            Step::Variant(variant) => {
                level = level.select(*variant)?;
                level_expr = Expr::variant(level_expr, *variant);
            }
            Step::Field(index) => {
                let field = level.field_at(*index)?;

                match &field.ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let key_expr = key_expr(&level, belongs_to, &level_expr)?;
                        return Some(EmbeddedRelationRef {
                            belongs_to,
                            key_expr,
                            tail: steps[i + 1..].to_vec(),
                        });
                    }
                    FieldTy::Embedded(embedded) => {
                        level = Level::embed(cx, embedded.target)?;
                        level_expr = Expr::project(level_expr, [*index]);
                    }
                    _ => return None,
                }
            }
        }
    }

    None
}

/// One embedded level of a resolution walk: the embed's model, with the
/// variant selected so far when it is an enum.
enum Level<'a> {
    Struct(&'a app::EmbeddedStruct),
    Enum(&'a app::EmbeddedEnum, Option<VariantId>),
}

impl<'a> Level<'a> {
    fn embed(cx: &ExprContext<'a>, model_id: app::ModelId) -> Option<Self> {
        match cx.schema().app.model(model_id) {
            app::Model::EmbeddedStruct(embedded) => Some(Level::Struct(embedded)),
            app::Model::EmbeddedEnum(embedded) => Some(Level::Enum(embedded, None)),
            app::Model::Root(_) => None,
        }
    }

    /// Apply a variant selection: only an enum level with no variant selected
    /// yet accepts one, and only for its own variants.
    fn select(self, variant: VariantId) -> Option<Self> {
        match self {
            Level::Enum(embedded, None)
                if embedded.id == variant.model && variant.index < embedded.variants.len() =>
            {
                Some(Level::Enum(embedded, Some(variant)))
            }
            _ => None,
        }
    }

    /// The field a step selects at this level: a struct field, or a field of
    /// the selected variant by its variant-local position.
    fn field_at(&self, index: usize) -> Option<&'a app::Field> {
        match self {
            Level::Struct(embedded) => embedded.fields.get(index),
            Level::Enum(embedded, Some(variant)) => {
                embedded.variant_fields(variant.index).nth(index)
            }
            Level::Enum(_, None) => None,
        }
    }

    /// The step selecting a field at this level, by its index in the embed's
    /// (flattened) field list.
    fn step_of(&self, field_index: usize) -> Option<usize> {
        match self {
            Level::Struct(_) => Some(field_index),
            Level::Enum(embedded, Some(variant)) => embedded
                .variant_fields(variant.index)
                .position(|field| field.id.index == field_index),
            Level::Enum(_, None) => None,
        }
    }
}

/// Build the expression projecting `belongs_to`'s key field(s) out of the
/// host row. The key fields are siblings of the relation at `level`, reached
/// by `level_expr`.
fn key_expr(level: &Level<'_>, belongs_to: &BelongsTo, level_expr: &Expr) -> Option<Expr> {
    let mut fields = Vec::with_capacity(belongs_to.foreign_key.fields.len());

    for fk_field in &belongs_to.foreign_key.fields {
        let step = level.step_of(fk_field.source.index)?;
        fields.push(Expr::project(level_expr.clone(), [step]));
    }

    Some(if fields.len() == 1 {
        fields.into_iter().next().unwrap()
    } else {
        Expr::record(fields)
    })
}
