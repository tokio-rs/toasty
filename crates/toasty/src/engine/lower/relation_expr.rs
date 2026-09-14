//! Resolve the first relation in an application-level path expression.
//!
//! Lookup walks `Project` and `Variant` nodes without constructing expressions.
//! The result borrows the input and records where the relation occurs, including
//! its position within a numeric projection. Callers can then build the host's
//! embedded foreign key or re-root the remaining expression at the target model.
//!
//! Variant selections identify fields but do not check discriminants. Statement
//! normalization supplies the guards; expression construction preserves the
//! selections those guards apply to.

use toasty_core::{
    schema::app::{self, BelongsTo, FieldTy, VariantId},
    stmt::{self, Expr, ExprContext, ResolvedRef},
};

/// A relation and its position in a borrowed path expression.
pub(super) struct ResolvedRelation<'s, 'e> {
    pub(super) field: &'s app::Field,
    pub(super) target: app::ModelId,
    // The containing embedded type, including its selected variant, if any.
    level: Option<Level<'s>>,
    expr: &'e Expr,
    // The reference or projection containing the relation field.
    at: &'e Expr,
    // For a projection, the number of steps up to and including the relation.
    offset: usize,
    has_target: bool,
}

impl<'s> ResolvedRelation<'s, '_> {
    pub(super) fn is_endpoint(&self) -> bool {
        !self.has_target
    }

    pub(super) fn embedded(&self) -> Option<&'s BelongsTo> {
        self.level.as_ref()?;
        match &self.field.ty {
            FieldTy::BelongsTo(relation) => Some(relation),
            _ => unreachable!("only belongs_to relations occur in embeds"),
        }
    }

    /// Build the embedded relation's sibling key fields on the host row.
    pub(super) fn key_expr(&self) -> Option<Expr> {
        let level = self.level.as_ref()?;
        let belongs_to = self.embedded()?;
        let Expr::Project(projection) = self.at else {
            unreachable!("embedded relations occur inside projections");
        };
        let host = project(
            copy_path(&projection.base),
            &projection.projection.as_slice()[..self.offset - 1],
        );
        let mut fields = Vec::with_capacity(belongs_to.foreign_key.fields.len());
        for fk in &belongs_to.foreign_key.fields {
            fields.push(Expr::project(
                host.clone(),
                [level.step_of(fk.source.index)?],
            ));
        }
        Some(if fields.len() == 1 {
            fields.remove(0)
        } else {
            Expr::record(fields)
        })
    }

    /// Build the part after the relation in the target model's scope.
    /// Returns `None` when the path ends at the relation.
    pub(super) fn target_expr(&self) -> Option<Expr> {
        if self.is_endpoint() {
            return None;
        }
        self.reroot(self.expr)
    }

    fn reroot(&self, expr: &Expr) -> Option<Expr> {
        if std::ptr::eq(expr, self.at) {
            return match expr {
                Expr::Project(projection) => {
                    self.project_target(None, &projection.projection.as_slice()[self.offset..])
                }
                _ => None,
            };
        }
        match expr {
            Expr::Project(projection) => self.project_target(
                self.reroot(&projection.base),
                projection.projection.as_slice(),
            ),
            Expr::Variant(variant) => {
                Some(Expr::variant(self.reroot(&variant.base)?, variant.variant))
            }
            _ => unreachable!("resolved paths contain only references, projections and variants"),
        }
    }

    fn project_target(&self, base: Option<Expr>, steps: &[usize]) -> Option<Expr> {
        match base {
            Some(base) => Some(project(base, steps)),
            None => {
                let (head, tail) = steps.split_first()?;
                Some(project(
                    Expr::ref_self_field(self.target.field(*head)),
                    tail,
                ))
            }
        }
    }
}

/// Locate the first direct or embedded relation without copying the path.
pub(super) fn resolve<'s, 'e>(
    cx: &ExprContext<'s>,
    expr: &'e Expr,
) -> Option<ResolvedRelation<'s, 'e>> {
    match resolve_path(cx, expr)? {
        PathTarget::Relation(relation) => Some(relation),
        PathTarget::Embed(_) => None,
    }
}

enum PathTarget<'s, 'e> {
    Embed(Level<'s>),
    Relation(ResolvedRelation<'s, 'e>),
}

fn resolve_path<'s, 'e>(cx: &ExprContext<'s>, expr: &'e Expr) -> Option<PathTarget<'s, 'e>> {
    match expr {
        Expr::Reference(reference @ stmt::ExprReference::Field { .. }) => {
            let ResolvedRef::Field(field) = cx.resolve_expr_reference(reference) else {
                return None;
            };
            if let FieldTy::Embedded(embedded) = &field.ty {
                Some(PathTarget::Embed(Level::embed(
                    &cx.schema().app,
                    embedded.target,
                )?))
            } else {
                Some(PathTarget::Relation(ResolvedRelation {
                    field,
                    target: field.relation_target_id()?,
                    level: None,
                    expr,
                    at: expr,
                    offset: 0,
                    has_target: false,
                }))
            }
        }
        Expr::Project(projection) => {
            let steps = projection.projection.as_slice();
            let mut level = match resolve_path(cx, &projection.base)? {
                PathTarget::Relation(mut relation) => {
                    relation.expr = expr;
                    relation.has_target |= !steps.is_empty();
                    return Some(PathTarget::Relation(relation));
                }
                PathTarget::Embed(level) => level,
            };
            for (i, index) in steps.iter().enumerate() {
                let field = level.field_at(*index)?;
                match &field.ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        return Some(PathTarget::Relation(ResolvedRelation {
                            field,
                            target: belongs_to.target,
                            level: Some(level),
                            expr,
                            at: expr,
                            offset: i + 1,
                            has_target: i + 1 < steps.len(),
                        }));
                    }
                    FieldTy::Embedded(embedded) => {
                        level = Level::embed(&cx.schema().app, embedded.target)?;
                    }
                    _ => return None,
                }
            }
            Some(PathTarget::Embed(level))
        }
        Expr::Variant(variant) => match resolve_path(cx, &variant.base)? {
            PathTarget::Embed(level) => Some(PathTarget::Embed(level.select(variant.variant)?)),
            PathTarget::Relation(mut relation) => {
                // A target model itself is not an enum; selection requires a
                // preceding projection into one of its fields.
                if relation.is_endpoint() {
                    return None;
                }
                relation.expr = expr;
                Some(PathTarget::Relation(relation))
            }
        },
        _ => None,
    }
}

/// Copy a path on demand, merging adjacent numeric projections so foreign
/// keys compare equally regardless of the input's projection boundaries.
fn copy_path(expr: &Expr) -> Expr {
    match expr {
        Expr::Project(projection) => project(
            copy_path(&projection.base),
            projection.projection.as_slice(),
        ),
        Expr::Variant(variant) => Expr::variant(copy_path(&variant.base), variant.variant),
        _ => expr.clone(),
    }
}

fn project(mut base: Expr, steps: &[usize]) -> Expr {
    if let Expr::Project(projection) = &mut base {
        for step in steps {
            projection.projection.push(*step);
        }
        base
    } else if steps.is_empty() {
        base
    } else {
        Expr::project(base, steps)
    }
}

/// One embedded level of a path walk: the embed's model, with the variant
/// selected so far when it is an enum.
pub(crate) enum Level<'a> {
    Struct(&'a app::EmbeddedStruct),
    Enum(&'a app::EmbeddedEnum, Option<VariantId>),
}

impl<'a> Level<'a> {
    /// The level for the embedded model `model_id`; `None` for a root model.
    pub(crate) fn embed(schema: &'a app::Schema, model_id: app::ModelId) -> Option<Self> {
        match schema.model(model_id) {
            app::Model::EmbeddedStruct(embedded) => Some(Level::Struct(embedded)),
            app::Model::EmbeddedEnum(embedded) => Some(Level::Enum(embedded, None)),
            app::Model::Root(_) => None,
        }
    }

    /// Apply a variant selection: only an enum level with no variant selected
    /// yet accepts one, and only for its own variants.
    pub(crate) fn select(self, variant: VariantId) -> Option<Self> {
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
    /// the selected variant by its variant-local position. An enum with no
    /// variant selected has no addressable fields: its record layout is a
    /// lowering concern, and a path names the variant it enters.
    pub(crate) fn field_at(&self, index: usize) -> Option<&'a app::Field> {
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
