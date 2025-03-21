use super::*;

use stmt::{Expr, Value};

impl Simplify<'_> {
    pub(super) fn simplify_expr_in_list(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
        // First, if the list is empty, then simplify to false
        if expr.list.is_list_empty() {
            return Some(Expr::Value(Value::Bool(false)));
        }

        self.rewrite_expr_in_list_when_model(expr);

        // Rewrite single-item lists into equalities
        self.rewrite_expr_in_list_with_single_item(expr)
    }

    fn rewrite_expr_in_list_when_model(&self, expr: &mut stmt::ExprInList) {
        if let stmt::Expr::Key(expr_key) = &mut *expr.expr {
            let model = self.schema.app.model(expr_key.model);

            let [pk_field_id] = &model.primary_key.fields[..] else {
                todo!()
            };
            let pk = self.schema.app.field(*pk_field_id);

            // Check RHS format
            match &mut *expr.list {
                stmt::Expr::List(expr_list) => {
                    for expr in &mut expr_list.items {
                        match expr {
                            stmt::Expr::Value(value) => {
                                assert!(value.is_a(&pk.ty.expect_primitive().ty));
                            }
                            _ => todo!("{expr:#?}"),
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

            *expr.expr = stmt::Expr::field(pk.id());
        }
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
