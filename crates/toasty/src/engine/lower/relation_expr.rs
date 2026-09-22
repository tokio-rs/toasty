//! Resolves the first relation in an application-level path expression.
//!
//! Resolution walks field references, projections, and embedded-enum variant
//! selections directly. It separates the path into the expression that reaches
//! an embedded relation and the expression that continues from its target model.
//!
//! Variant selections identify fields but do not test the discriminant. Statement
//! normalization adds the corresponding variant predicates.

use toasty_core::{
    schema::app::{self, BelongsTo, FieldTy, VariantId},
    stmt::{Expr, ExprContext, ExprReference, ResolvedRef},
};

/// A relation found in an application-level expression.
pub(super) struct ResolvedRelation<'a> {
    pub(super) field: &'a app::Field,
    pub(super) target: app::ModelId,
    embedded: Option<EmbeddedRelation<'a>>,
    target_expr: Option<Expr>,
}

struct EmbeddedRelation<'a> {
    belongs_to: &'a BelongsTo,
    key_expr: Expr,
}

impl<'a> ResolvedRelation<'a> {
    fn new(field: &'a app::Field, embedded: Option<EmbeddedRelation<'a>>) -> Option<Self> {
        let target = field.relation_target_id()?;

        Some(Self {
            field,
            target,
            embedded,
            target_expr: None,
        })
    }

    fn project(&mut self, indices: &[usize]) {
        let Some((index, remaining)) = indices.split_first() else {
            return;
        };
        self.target_expr = Some(match self.target_expr.take() {
            Some(base) => project(base, indices),
            None => project(Expr::ref_self_field(self.target.field(*index)), remaining),
        });
    }

    pub(super) fn is_endpoint(&self) -> bool {
        self.target_expr.is_none()
    }

    pub(super) fn embedded(&self) -> Option<&'a BelongsTo> {
        self.embedded.as_ref().map(|relation| relation.belongs_to)
    }

    /// Returns the embedded relation's foreign key on the host record.
    pub(super) fn key_expr(&self) -> Option<Expr> {
        self.embedded
            .as_ref()
            .map(|relation| relation.key_expr.clone())
    }

    /// Returns the part of the path evaluated against the target model.
    pub(super) fn target_expr(&self) -> Option<Expr> {
        self.target_expr.clone()
    }
}

/// Locates the first direct or embedded relation in `expr`.
pub(super) fn resolve<'a>(cx: &ExprContext<'a>, expr: &Expr) -> Option<ResolvedRelation<'a>> {
    match resolve_expr(cx, expr)? {
        PathTarget::Relation(relation) => Some(relation),
        PathTarget::Embed(_) => None,
    }
}

enum PathTarget<'a> {
    Embed(EmbedTarget<'a>),
    Relation(ResolvedRelation<'a>),
}

fn resolve_expr<'a>(cx: &ExprContext<'a>, expr: &Expr) -> Option<PathTarget<'a>> {
    match expr {
        Expr::Reference(reference @ ExprReference::Field { .. }) => {
            let ResolvedRef::Field(field) = cx.resolve_expr_reference(reference) else {
                return None;
            };
            match &field.ty {
                FieldTy::Embedded(embedded) => Some(PathTarget::Embed(EmbedTarget::embed(
                    &cx.schema().app,
                    embedded.target,
                )?)),
                _ => Some(PathTarget::Relation(ResolvedRelation::new(field, None)?)),
            }
        }
        Expr::Project(projection) => {
            let indices = projection.projection.as_slice();
            let mut target = match resolve_expr(cx, &projection.base)? {
                PathTarget::Relation(mut relation) => {
                    relation.project(indices);
                    return Some(PathTarget::Relation(relation));
                }
                PathTarget::Embed(target) => target,
            };
            for (position, index) in indices.iter().enumerate() {
                let field = target.field_at(*index)?;

                match &field.ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let host = project(copy_path(&projection.base), &indices[..position]);
                        let key_expr = embedded_key_expr(host, target, belongs_to)?;

                        let mut relation = ResolvedRelation::new(
                            field,
                            Some(EmbeddedRelation {
                                belongs_to,
                                key_expr,
                            }),
                        )?;
                        relation.project(&indices[position + 1..]);
                        return Some(PathTarget::Relation(relation));
                    }
                    FieldTy::Embedded(embedded) => {
                        target = EmbedTarget::embed(&cx.schema().app, embedded.target)?;
                    }
                    _ => return None,
                }
            }
            Some(PathTarget::Embed(target))
        }
        Expr::Variant(variant) => match resolve_expr(cx, &variant.base)? {
            PathTarget::Embed(target) => Some(PathTarget::Embed(target.select(variant.variant)?)),
            PathTarget::Relation(mut relation) => {
                // A relation targets a model; a variant requires a target field first.
                relation.target_expr =
                    Some(Expr::variant(relation.target_expr.take()?, variant.variant));
                Some(PathTarget::Relation(relation))
            }
        },
        _ => None,
    }
}

fn embedded_key_expr(host: Expr, target: EmbedTarget<'_>, belongs_to: &BelongsTo) -> Option<Expr> {
    let mut fields = Vec::with_capacity(belongs_to.foreign_key.fields.len());

    for foreign_key in &belongs_to.foreign_key.fields {
        let index = target.step_of(foreign_key.source.index)?;
        fields.push(Expr::project(host.clone(), [index]));
    }

    Some(scalar_or_record(fields))
}

fn scalar_or_record(mut fields: Vec<Expr>) -> Expr {
    if fields.len() == 1 {
        fields.pop().unwrap()
    } else {
        Expr::record(fields)
    }
}

/// Copies the host path once a relation is found, merging adjacent projections.
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

fn project(mut base: Expr, indices: &[usize]) -> Expr {
    if let Expr::Project(project) = &mut base {
        for index in indices {
            project.projection.push(*index);
        }
        base
    } else if indices.is_empty() {
        base
    } else {
        Expr::project(base, indices)
    }
}

/// The embedded struct or enum, with an optional selected variant, reached by a path.
#[derive(Clone, Copy)]
pub(crate) enum EmbedTarget<'a> {
    Struct(&'a app::EmbeddedStruct),
    Enum(&'a app::EmbeddedEnum, Option<VariantId>),
}

impl<'a> EmbedTarget<'a> {
    /// Returns the embedded model for `model_id`.
    pub(crate) fn embed(schema: &'a app::Schema, model_id: app::ModelId) -> Option<Self> {
        match schema.model(model_id) {
            app::Model::EmbeddedStruct(embedded) => Some(EmbedTarget::Struct(embedded)),
            app::Model::EmbeddedEnum(embedded) => Some(EmbedTarget::Enum(embedded, None)),
            app::Model::Root(_) => None,
        }
    }

    /// Selects a variant of the current embedded enum.
    pub(crate) fn select(self, variant: VariantId) -> Option<Self> {
        match self {
            EmbedTarget::Enum(embedded, None)
                if embedded.id == variant.model && variant.index < embedded.variants.len() =>
            {
                Some(EmbedTarget::Enum(embedded, Some(variant)))
            }
            _ => None,
        }
    }

    /// Returns the field selected by a local path index.
    pub(crate) fn field_at(&self, index: usize) -> Option<&'a app::Field> {
        match self {
            EmbedTarget::Struct(embedded) => embedded.fields.get(index),
            EmbedTarget::Enum(embedded, Some(variant)) => {
                embedded.variant_fields(variant.index).get(index)
            }
            EmbedTarget::Enum(_, None) => None,
        }
    }

    /// Returns a field's local index within this target.
    fn step_of(&self, field_index: usize) -> Option<usize> {
        match self {
            EmbedTarget::Struct(_) => Some(field_index),
            EmbedTarget::Enum(embedded, Some(variant)) => embedded
                .variant_fields(variant.index)
                .iter()
                .position(|field| field.id.index == field_index),
            EmbedTarget::Enum(_, None) => None,
        }
    }
}
