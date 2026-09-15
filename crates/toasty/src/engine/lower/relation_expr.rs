//! Resolves the first relation in an application-level path expression.
//!
//! A path consists of a field reference followed by field projections and
//! embedded-enum variant selections. Resolution separates the path into the
//! expression that reaches an embedded relation and the expression that
//! continues from the relation's target model.
//!
//! Variant steps select fields but do not test the discriminant. Statement
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
    fn new(
        field: &'a app::Field,
        embedded: Option<EmbeddedRelation<'a>>,
        target_steps: &[PathStep],
    ) -> Option<Self> {
        let target = field.relation_target_id()?;
        let target_expr = match target_steps.split_first() {
            None => None,
            Some((PathStep::Field(index), remaining)) => {
                let root = Expr::ref_self_field(target.field(*index));
                Some(apply_steps(root, remaining))
            }
            Some((PathStep::Variant(_), _)) => return None,
        };

        Some(Self {
            field,
            target,
            embedded,
            target_expr,
        })
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
    let path = ExprPath::parse(expr)?;
    let ResolvedRef::Field(field) = cx.resolve_expr_reference(&path.root) else {
        return None;
    };

    match &field.ty {
        FieldTy::Embedded(embedded) => resolve_embedded(cx, path, embedded.target),
        _ => ResolvedRelation::new(field, None, &path.steps),
    }
}

fn resolve_embedded<'a>(
    cx: &ExprContext<'a>,
    path: ExprPath,
    model: app::ModelId,
) -> Option<ResolvedRelation<'a>> {
    let mut target = EmbedTarget::embed(&cx.schema().app, model)?;

    for (position, step) in path.steps.iter().enumerate() {
        match step {
            PathStep::Variant(variant) => target = target.select(*variant)?,
            PathStep::Field(index) => {
                let field = target.field_at(*index)?;

                match &field.ty {
                    FieldTy::BelongsTo(belongs_to) => {
                        let host = path.expr_before(position);
                        let key_expr = embedded_key_expr(host, target, belongs_to)?;

                        return ResolvedRelation::new(
                            field,
                            Some(EmbeddedRelation {
                                belongs_to,
                                key_expr,
                            }),
                            path.steps_after(position),
                        );
                    }
                    FieldTy::Embedded(embedded) => {
                        target = EmbedTarget::embed(&cx.schema().app, embedded.target)?;
                    }
                    _ => return None,
                }
            }
        }
    }

    None
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

/// A parsed expression path, before schema resolution.
struct ExprPath {
    root: ExprReference,
    steps: Vec<PathStep>,
}

impl ExprPath {
    fn parse(expr: &Expr) -> Option<Self> {
        let mut steps = vec![];
        let root = parse_steps(expr, &mut steps)?;

        Some(Self { root, steps })
    }

    fn expr_before(&self, position: usize) -> Expr {
        apply_steps(self.root.into(), &self.steps[..position])
    }

    fn steps_after(&self, position: usize) -> &[PathStep] {
        &self.steps[position + 1..]
    }
}

#[derive(Clone, Copy)]
enum PathStep {
    Field(usize),
    Variant(VariantId),
}

fn parse_steps(expr: &Expr, steps: &mut Vec<PathStep>) -> Option<ExprReference> {
    match expr {
        Expr::Reference(reference @ ExprReference::Field { .. }) => Some(*reference),
        Expr::Project(project) => {
            let root = parse_steps(&project.base, steps)?;
            steps.extend(project.projection.iter().copied().map(PathStep::Field));
            Some(root)
        }
        Expr::Variant(variant) => {
            let root = parse_steps(&variant.base, steps)?;
            steps.push(PathStep::Variant(variant.variant));
            Some(root)
        }
        _ => None,
    }
}

fn apply_steps(mut expr: Expr, steps: &[PathStep]) -> Expr {
    for step in steps {
        match step {
            PathStep::Field(index) => expr = project(expr, *index),
            PathStep::Variant(variant) => expr = Expr::variant(expr, *variant),
        }
    }
    expr
}

fn project(mut base: Expr, index: usize) -> Expr {
    if let Expr::Project(project) = &mut base {
        project.projection.push(index);
        base
    } else {
        Expr::project(base, [index])
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
