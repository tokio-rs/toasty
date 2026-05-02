use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    /// Heavyweight `IN`-list rewrites. Cheap canonicalization (empty list,
    /// null propagation, literal dedup, single-item collapse) runs in
    /// `fold::expr_in_list` before this is reached.
    pub(super) fn simplify_expr_in_list(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        // Currently only the model → primary-key-field rewrite is heavyweight.
        // It is in-place and does not produce a top-level expression replacement.
        self.rewrite_expr_in_list_when_model(expr);
        None
    }

    fn rewrite_expr_in_list_when_model(&self, expr: &mut stmt::ExprInList) {
        let (nesting, pk_field_id) = {
            let stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Model { nesting }) =
                &*expr.expr
            else {
                return;
            };
            let nesting = *nesting;
            let model = self.cx.resolve_expr_reference(expr_ref).as_model_unwrap();
            let [pk_field_id] = &model.primary_key.fields[..] else {
                todo!()
            };
            (nesting, *pk_field_id)
        };

        let pk = self.field(pk_field_id);

        // Check RHS format
        match &mut *expr.list {
            stmt::Expr::List(expr_list) => {
                for item in &mut expr_list.items {
                    match item {
                        stmt::Expr::Value(value) => {
                            assert!(value.is_a(&pk.ty.as_primitive_unwrap().ty));
                        }
                        _ => todo!("{item:#?}"),
                    }
                }
            }
            stmt::Expr::Value(stmt::Value::List(values)) => {
                for value in values {
                    assert!(value.is_a(&pk.ty.as_primitive_unwrap().ty));
                }
            }
            _ => todo!("expr={expr:#?}"),
        }

        *expr.expr = stmt::Expr::ref_field(nesting, pk.id());
    }
}
