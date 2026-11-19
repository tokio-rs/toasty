use super::Simplify;
use toasty_core::schema::app::FieldTy;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Heavyweight `CAST` rewrites. Cheap canonicalization (constant
    /// folding) runs in `fold::expr_cast` before this is reached.
    pub(super) fn simplify_expr_cast(&self, expr: &mut stmt::ExprCast) -> Option<stmt::Expr> {
        // Root model values retain keys until lowering selects the relation's
        // referenced fields. Embedded records retain relation slots until
        // relation planning has assigned their foreign keys.
        if expr.from.is_none()
            && let stmt::Type::Model(model) = expr.ty
            && expr.expr.record_len().is_some()
        {
            let model = self.cx.schema().app.model(model);
            if let Some(model) = model.as_root() {
                for field in &model.fields {
                    if !model.primary_key.fields.contains(&field.id)
                        && !model.referenced_fields.contains(&field.id)
                    {
                        expr.expr.entry_mut(field.id.index).take();
                    }
                }
                return None;
            }
            if model.fields().iter().any(|field| field.ty.is_belongs_to()) {
                return None;
            }
        }

        // Schema-directed constant folding: a `#[document]` cast (marked by a
        // source type or a model-typed target) resolves the embed's field
        // names through the schema, so the schema-free fold pass skips it and
        // it folds here instead. The operand may be a constant expression
        // *tree* (an insert row's document value is an `Expr::Record` of
        // per-leaf values), so it is folded to a value first.
        if expr.from.is_some() || expr.ty.contains_model() {
            if !expr.expr.is_const() {
                return None;
            }
            let value = expr
                .expr
                .eval_const()
                .expect("constant expression failed to evaluate");
            let value = expr
                .ty
                .cast_from(self.cx.schema(), expr.from.as_ref(), value)
                .expect("failed to cast value");
            return Some(value.into());
        }

        // Redundant cast elimination: `cast(x as T) → x` when x is already T.
        //
        // If the inner expression is a field reference whose primitive type
        // matches the cast target, the cast is a no-op and can be removed.
        let stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) = &*expr.expr else {
            return None;
        };

        let field = self.cx.resolve_expr_reference(f).as_field_unwrap();
        let FieldTy::Primitive(primitive) = &field.ty else {
            return None;
        };

        if primitive.ty == expr.ty {
            return Some(expr.expr.take());
        }

        None
    }
}
