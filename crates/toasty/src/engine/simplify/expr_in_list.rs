use super::Simplify;
use std::collections::HashSet;
use toasty_core::stmt::{self, Expr, Value};

impl Simplify<'_> {
    pub(super) fn simplify_expr_in_list(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        // `x in ()` → `false`
        if expr.list.is_list_empty() {
            return Some(Expr::Value(Value::Bool(false)));
        }

        // Null propagation, `null in (x, y, z)` → `null`
        if expr.expr.is_value_null() {
            return Some(Expr::null());
        }

        self.rewrite_expr_in_list_when_model(expr);

        // Deduplicate literal value lists: `x in (1, 1, 2)` → `x in (1, 2)`
        self.dedup_expr_in_list_values(expr);

        // Rewrite single-item lists into equalities
        self.rewrite_expr_in_list_with_single_item(expr)
    }

    /// Remove duplicate values from a `Value::List` in-place, preserving order.
    fn dedup_expr_in_list_values(&self, expr: &mut stmt::ExprInList) {
        if let Expr::Value(Value::List(values)) = &mut *expr.list {
            let mut seen = HashSet::new();
            values.retain(|v| seen.insert(v.clone()));
        }
    }

    fn rewrite_expr_in_list_when_model(&self, expr: &mut stmt::ExprInList) {
        let (nesting, pk_field_id) = {
            let stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Model { nesting }) =
                &*expr.expr
            else {
                return;
            };
            let nesting = *nesting;
            let model = self.cx.resolve_expr_reference(expr_ref).expect_model();
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
                            assert!(value.is_a(&pk.ty.expect_primitive().ty));
                        }
                        _ => todo!("{item:#?}"),
                    }
                }
            }
            stmt::Expr::Value(stmt::Value::List(values)) => {
                for value in values {
                    assert!(value.is_a(&pk.ty.expect_primitive().ty));
                }
            }
            _ => todo!("expr={expr:#?}"),
        }

        *expr.expr = stmt::Expr::ref_field(nesting, pk.id());
    }

    fn rewrite_expr_in_list_with_single_item(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        let rhs = match &mut *expr.list {
            Expr::Value(value) => {
                let values = match value {
                    Value::List(value) => &value[..],
                    _ => todo!("{value:#?}"),
                };

                if values.len() != 1 {
                    return None;
                }

                Expr::Value(values[0].clone())
            }
            Expr::List(expr_list) => {
                if expr_list.items.len() != 1 {
                    return None;
                }

                expr_list.items[0].take()
            }
            Expr::Record(_) => todo!("should not happen"),
            _ => return None,
        };

        Some(Expr::eq(expr.expr.take(), rhs))
    }
}
