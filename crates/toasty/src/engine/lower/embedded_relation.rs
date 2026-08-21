//! Resolution of relation references reached through embedded types.
//!
//! A filter on a relation stored in an embedded type arrives as a projection
//! into the embed's record — `Project(Ref(owner), [slot, ...])` — possibly
//! scoped by `is_variant` gates when the embed is an enum. This module
//! resolves such a projection to the `belongs_to` it lands on (or passes
//! through) and builds the expression projecting the relation's key field(s)
//! out of the host row, which is what the [`lift_in_subquery`] rewrites
//! substitute for the relation reference.
//!
//! Enum slot indices are ambiguous on their own: record slots are
//! variant-local (`[discriminant, variant fields...]`), so two variants place
//! different fields at the same slot. The typed layer always fuses an
//! `is_variant` gate next to a variant-rooted comparison (`Path::build_filter`,
//! `matches`), and resolution uses those gates as the variant context — a
//! projection into an enum with no matching gate does not resolve, and no
//! rewrite fires.
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

    /// Projection steps remaining after the relation field — a field path
    /// into the relation's target model. Empty when the expression
    /// references the relation itself.
    pub(super) tail: Vec<usize>,
}

/// An `is_variant` gate found alongside the expression being resolved,
/// normalized to the flattened location of the enum it scopes.
pub(super) struct VariantGate {
    base: stmt::ExprReference,
    steps: Vec<usize>,
    variant: VariantId,
}

/// Collect the `is_variant` gates among an `And`'s operands, normalized for
/// [`resolve_embedded_relation`] lookups.
pub(super) fn collect_variant_gates(operands: &[Expr]) -> Vec<VariantGate> {
    operands
        .iter()
        .filter_map(|operand| {
            let Expr::IsVariant(is_variant) = operand else {
                return None;
            };
            let (base, steps) = flatten_projection(&is_variant.expr)?;
            Some(VariantGate {
                base,
                steps,
                variant: is_variant.variant,
            })
        })
        .collect()
}

/// Resolve `expr` — a projection into an embedded type — to the `belongs_to`
/// its path lands on or passes through.
///
/// Returns `None` when the expression is not such a projection: the base is
/// not an embedded field, a step lands on a non-relation leaf, or an enum
/// step has no scoping gate in `gates`.
pub(super) fn resolve_embedded_relation<'a>(
    cx: &ExprContext<'a>,
    expr: &Expr,
    gates: &[VariantGate],
) -> Option<EmbeddedRelationRef<'a>> {
    let (base, steps) = flatten_projection(expr)?;

    let ResolvedRef::Field(base_field) = cx.resolve_expr_reference(&base) else {
        return None;
    };
    let FieldTy::Embedded(embedded) = &base_field.ty else {
        return None;
    };

    let mut model_id = embedded.target;
    // Slots consumed so far, i.e. the location of the current embed level
    // relative to the base reference.
    let mut prefix: Vec<usize> = vec![];

    for (i, &slot) in steps.iter().enumerate() {
        let level = Level::at(cx, model_id, &base, &prefix, gates)?;
        let field = level.field_at(slot)?;

        match &field.ty {
            FieldTy::BelongsTo(belongs_to) => {
                let key_expr = key_expr(&level, belongs_to, &base, &prefix)?;
                return Some(EmbeddedRelationRef {
                    belongs_to,
                    key_expr,
                    tail: steps[i + 1..].to_vec(),
                });
            }
            FieldTy::Embedded(embedded) => {
                model_id = embedded.target;
                prefix.push(slot);
            }
            _ => return None,
        }
    }

    None
}

/// One embedded level of a resolution walk: the embed's model, with the
/// variant scope applied when it is an enum.
enum Level<'a> {
    Struct(&'a app::EmbeddedStruct),
    Enum(&'a app::EmbeddedEnum, VariantId),
}

impl<'a> Level<'a> {
    fn at(
        cx: &ExprContext<'a>,
        model_id: app::ModelId,
        base: &stmt::ExprReference,
        prefix: &[usize],
        gates: &[VariantGate],
    ) -> Option<Self> {
        match cx.schema().app.model(model_id) {
            app::Model::EmbeddedStruct(embedded) => Some(Level::Struct(embedded)),
            app::Model::EmbeddedEnum(embedded) => {
                let gate = gates.iter().find(|gate| {
                    gate.variant.model == model_id && gate.base == *base && gate.steps == prefix
                })?;
                Some(Level::Enum(embedded, gate.variant))
            }
            app::Model::Root(_) => None,
        }
    }

    /// The field a record-slot index selects at this level. Enum slot 0 is
    /// the discriminant, so variant fields start at slot 1.
    fn field_at(&self, slot: usize) -> Option<&'a app::Field> {
        match self {
            Level::Struct(embedded) => embedded.fields.get(slot),
            Level::Enum(embedded, variant) => embedded
                .variant_fields(variant.index)
                .nth(slot.checked_sub(1)?),
        }
    }

    /// The record-slot index of a field at this level, by its index in the
    /// embed's (flattened) field list.
    fn slot_of(&self, field_index: usize) -> Option<usize> {
        match self {
            Level::Struct(_) => Some(field_index),
            Level::Enum(embedded, variant) => embedded
                .variant_fields(variant.index)
                .position(|field| field.id.index == field_index)
                .map(|local| local + 1),
        }
    }
}

/// Build the expression projecting `belongs_to`'s key field(s) out of the
/// host row. The key fields are siblings of the relation at `level`, located
/// at `prefix` relative to `base`.
fn key_expr(
    level: &Level<'_>,
    belongs_to: &BelongsTo,
    base: &stmt::ExprReference,
    prefix: &[usize],
) -> Option<Expr> {
    let mut fields = Vec::with_capacity(belongs_to.foreign_key.fields.len());

    for fk_field in &belongs_to.foreign_key.fields {
        let slot = level.slot_of(fk_field.source.index)?;
        let mut slots = prefix.to_vec();
        slots.push(slot);
        fields.push(Expr::project(Expr::Reference(*base), slots.as_slice()));
    }

    Some(if fields.len() == 1 {
        fields.into_iter().next().unwrap()
    } else {
        Expr::record(fields)
    })
}

/// Peel nested `Project` layers, returning the base field reference and the
/// concatenated slot path (outermost step first). Variant-rooted typed paths
/// produce nested projections (`Project(Project(Ref, [a]), [b])`), so a
/// single `ExprProject` pattern-match is not enough.
fn flatten_projection(expr: &Expr) -> Option<(stmt::ExprReference, Vec<usize>)> {
    match expr {
        Expr::Project(project) => {
            let (base, mut steps) = flatten_projection(&project.base)?;
            steps.extend(project.projection.as_slice().iter().copied());
            Some((base, steps))
        }
        Expr::Reference(reference @ stmt::ExprReference::Field { .. }) => {
            Some((*reference, vec![]))
        }
        _ => None,
    }
}
