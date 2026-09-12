//! Resolve belongs-to references through embedded values. Foreign-key
//! expressions stay private: both comparison and subquery rewrites attach
//! every enclosing variant gate before returning a predicate to the caller.

use toasty_core::{
    schema::app::{self, FieldTy},
    stmt::{self, Expr, ExprContext, PathStep, VisitMut},
};

pub(super) struct Relation<'a> {
    pub field: &'a app::Field,
    pub path: Expr,
    pub tail: Vec<PathStep>,
    parent: Expr,
    variant: Option<app::VariantId>,
    gates: Vec<Expr>,
}

impl Relation<'_> {
    fn source(&self, schema: &app::Schema, field: app::FieldId) -> Expr {
        let index = match self.variant {
            Some(variant) => schema
                .model(variant.model)
                .as_embedded_enum_unwrap()
                .variant_fields(variant.index)
                .position(|f| f.id == field)
                .unwrap(),
            None => field.index,
        };
        let mut steps = vec![];
        if let Some(variant) = self.variant {
            steps.push(PathStep::Variant(variant));
        }
        steps.push(PathStep::Field(index));
        Expr::path(self.parent.clone(), steps)
    }

    fn gate(&self, expr: Expr) -> Expr {
        let mut operands = self.gates.clone();
        operands.push(expr);
        Expr::and_from_vec(operands)
    }

    pub fn compare(&self, schema: &app::Schema, op: stmt::BinaryOp, rhs: Expr) -> Expr {
        self.gate(Expr::binary_op(self.key(schema), op, rhs))
    }

    pub fn compare_relation(
        &self,
        schema: &app::Schema,
        op: stmt::BinaryOp,
        rhs: &Relation<'_>,
    ) -> Option<Expr> {
        let lhs_rel = self.field.ty.as_belongs_to_unwrap();
        let rhs_rel = rhs.field.ty.as_belongs_to_unwrap();
        if lhs_rel.target != rhs_rel.target
            || !lhs_rel
                .foreign_key
                .fields
                .iter()
                .map(|fk| fk.target)
                .eq(rhs_rel.foreign_key.fields.iter().map(|fk| fk.target))
        {
            return None;
        }
        Some(rhs.gate(self.compare(schema, op, rhs.key(schema))))
    }

    fn key(&self, schema: &app::Schema) -> Expr {
        let relation = self.field.ty.as_belongs_to_unwrap();
        let mut keys: Vec<_> = relation
            .foreign_key
            .fields
            .iter()
            .map(|fk| self.source(schema, fk.source))
            .collect();
        if keys.len() == 1 {
            keys.pop().unwrap()
        } else {
            Expr::record(keys)
        }
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

pub(super) fn resolve<'a>(cx: &ExprContext<'a>, expr: &Expr) -> Option<Relation<'a>> {
    let mut steps = vec![];
    let base = super::path::flatten(expr, &mut steps);
    let Expr::Reference(reference @ stmt::ExprReference::Field { .. }) = base else {
        return None;
    };
    let stmt::ResolvedRef::Field(mut field) = cx.resolve_expr_reference(reference) else {
        return None;
    };
    let mut path = base.clone();
    let mut gates = vec![];
    let mut selected = None;
    for (offset, step) in steps.iter().enumerate() {
        let FieldTy::Embedded(embed) = &field.ty else {
            return None;
        };
        let model = cx.schema().app.model(embed.target);
        let index = match step {
            PathStep::Variant(variant) => {
                assert_eq!(variant.model, model.id());
                gates.push(Expr::is_variant(path.clone(), *variant));
                selected = Some(*variant);
                continue;
            }
            PathStep::Field(index) => *index,
        };
        let parent = path.clone();
        let variant = selected.take();
        field = match variant {
            Some(variant) => model
                .as_embedded_enum_unwrap()
                .variant_fields(variant.index)
                .nth(index)?,
            None if matches!(model, app::Model::EmbeddedEnum(_)) => return None,
            None => model.fields().get(index)?,
        };
        let mut field_steps = vec![];
        if let Some(variant) = variant {
            field_steps.push(PathStep::Variant(variant));
        }
        field_steps.push(PathStep::Field(index));
        path = Expr::path(path, field_steps);
        if field.ty.is_belongs_to() {
            return Some(Relation {
                field,
                parent,
                path,
                variant,
                gates,
                tail: steps[offset + 1..].to_vec(),
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
