use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_expr_list(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        let mut all_values = true;

        for expr in &mut expr.items {
            all_values &= expr.is_value();
        }

        if all_values {
            let mut values = vec![];

            for expr in expr.items.drain(..) {
                let stmt::Expr::Value(value) = expr else {
                    panic!()
                };
                values.push(value);
            }

            Some(stmt::Value::list_from_vec(values).into())
        } else {
            None
        }
    }
}
