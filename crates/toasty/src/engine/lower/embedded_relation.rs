//! Resolve belongs-to references through embedded values. Foreign-key
//! expressions stay private: both comparison and subquery rewrites attach
//! every enclosing variant gate before returning a predicate to the caller.

use toasty_core::{
    schema::app::{self, FieldTy},
    stmt::{self, Expr, ExprContext, VisitMut},
};

pub(super) struct Relation<'a> {
    pub field: &'a app::Field,
    pub path: Expr,
    pub tail: Vec<usize>,
    parent: Expr,
    variant: Option<app::VariantId>,
    gates: Vec<Expr>,
}

impl Relation<'_> {
    fn source(&self, schema: &app::Schema, field: app::FieldId) -> Expr {
        let index = match self.variant {
            Some(variant) => {
                schema
                    .model(variant.model)
                    .as_embedded_enum_unwrap()
                    .variant_fields(variant.index)
                    .position(|f| f.id == field)
                    .unwrap()
                    + 1
            }
            None => field.index,
        };
        let mut expr = Expr::project(self.parent.clone(), [index]);
        if let Expr::Project(project) = &mut expr {
            project.variant = self.variant;
        }
        expr
    }

    fn gate(&self, expr: Expr) -> Expr {
        let mut operands = self.gates.clone();
        operands.push(expr);
        Expr::and_from_vec(operands)
    }

    pub fn compare(&self, schema: &app::Schema, op: stmt::BinaryOp, rhs: Expr) -> Expr {
        let relation = self.field.ty.as_belongs_to_unwrap();
        let mut keys: Vec<_> = relation
            .foreign_key
            .fields
            .iter()
            .map(|fk| self.source(schema, fk.source))
            .collect();
        let lhs = if keys.len() == 1 {
            keys.pop().unwrap()
        } else {
            Expr::record(keys)
        };
        self.gate(Expr::binary_op(lhs, op, rhs))
    }

    pub fn rebase(&self, schema: &app::Schema, mut expr: Expr) -> Expr {
        struct Rebase<'a, 'b> {
            relation: &'a Relation<'b>,
            schema: &'a app::Schema,
        }
        impl VisitMut for Rebase<'_, '_> {
            fn visit_expr_mut(&mut self, expr: &mut Expr) {
                if let Expr::Reference(stmt::ExprReference::Field { index, nesting: 0 }) = expr {
                    *expr = self
                        .relation
                        .source(self.schema, self.relation.field.id.model.field(*index));
                } else {
                    stmt::visit_mut::visit_expr_mut(self, expr);
                }
            }
            fn visit_stmt_query_mut(&mut self, _: &mut stmt::Query) {}
        }
        Rebase {
            relation: self,
            schema,
        }
        .visit_expr_mut(&mut expr);
        self.gate(expr)
    }
}

pub(super) fn flatten<'a>(
    expr: &'a Expr,
    steps: &mut Vec<(usize, Option<app::VariantId>)>,
) -> &'a Expr {
    if let Expr::Project(project) = expr {
        let base = flatten(&project.base, steps);
        steps.extend(
            project
                .projection
                .iter()
                .enumerate()
                .map(|(i, index)| (*index, if i == 0 { project.variant } else { None })),
        );
        base
    } else {
        expr
    }
}

pub(super) fn resolve<'a>(cx: &ExprContext<'a>, expr: &Expr) -> Option<Relation<'a>> {
    let mut steps = vec![];
    let base = flatten(expr, &mut steps);
    let Expr::Reference(reference @ stmt::ExprReference::Field { .. }) = base else {
        return None;
    };
    let stmt::ResolvedRef::Field(mut field) = cx.resolve_expr_reference(reference) else {
        return None;
    };
    let mut path = base.clone();
    let mut gates = vec![];
    for (offset, (index, variant)) in steps.iter().enumerate() {
        let FieldTy::Embedded(embed) = &field.ty else {
            return None;
        };
        let model = cx.schema().app.model(embed.target);
        let parent = path.clone();
        field = match variant {
            Some(variant) => {
                gates.push(Expr::is_variant(parent.clone(), *variant));
                model
                    .as_embedded_enum_unwrap()
                    .variant_fields(variant.index)
                    .nth(index.checked_sub(1)?)?
            }
            None if matches!(model, app::Model::EmbeddedEnum(_)) => return None,
            None => model.fields().get(*index)?,
        };
        path = Expr::project(path, [*index]);
        if let Expr::Project(project) = &mut path {
            project.variant = *variant;
        }
        if field.ty.is_belongs_to() {
            return Some(Relation {
                field,
                parent,
                path,
                variant: *variant,
                gates,
                tail: steps[offset + 1..]
                    .iter()
                    .map(|(index, _)| *index)
                    .collect(),
            });
        }
    }
    None
}

/// Move the referenced fields encoded by a loaded relation into its sibling
/// key slots. A null relation slot means unloaded and preserves explicit keys.
pub(super) fn rewrite_value(
    schema: &app::Schema,
    model: &app::Model,
    expr: &mut Expr,
) -> toasty_core::Result<()> {
    // Simplification can fold an embedded literal into a value record.
    if let Expr::Value(stmt::Value::Record(record)) = expr {
        *expr = Expr::record(
            std::mem::take(&mut record.fields)
                .into_iter()
                .map(Expr::Value),
        );
    }
    let Expr::Record(record) = expr else {
        return Ok(());
    };
    let (fields, offset): (Vec<_>, _) = match model {
        app::Model::EmbeddedEnum(model) => {
            let Some(Expr::Value(discriminant)) = record.fields.first() else {
                return Ok(());
            };
            let variant = model
                .variants
                .iter()
                .position(|v| &v.discriminant == discriminant)
                .unwrap();
            (model.variant_fields(variant).collect(), 1)
        }
        _ => (model.fields().iter().collect(), 0),
    };
    for (index, field) in fields.iter().enumerate() {
        match &field.ty {
            FieldTy::Embedded(embed) => rewrite_value(
                schema,
                schema.model(embed.target),
                &mut record.fields[index + offset],
            )?,
            FieldTy::BelongsTo(rel) if !model.is_root() => {
                let value = record.fields[index + offset].take();
                if value.is_value_null() || value.is_default() {
                    continue;
                }
                let values: Vec<_> = value
                    .into_record_items()
                    .expect("embedded relation must encode its referenced fields")
                    .collect();
                assert_eq!(values.len(), rel.foreign_key.fields.len());
                if values.iter().all(Expr::is_default) {
                    continue;
                }
                for (fk, value) in rel.foreign_key.fields.iter().zip(values) {
                    let index = fields
                        .iter()
                        .position(|field| field.id == fk.source)
                        .unwrap();
                    record.fields[index + offset] = value;
                }
            }
            _ => {}
        }
    }
    if !model.is_root() {
        for (index, field) in fields.iter().enumerate() {
            if !field.nullable
                && !field.is_relation()
                && record.fields[index + offset].is_value_null()
            {
                return Err(toasty_core::Error::validation_failed(format!(
                    "missing non-nullable embedded field `{}`",
                    field.name,
                )));
            }
        }
    }
    Ok(())
}
