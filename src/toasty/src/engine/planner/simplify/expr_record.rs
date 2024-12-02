use super::*;

impl SimplifyExpr<'_> {
    pub(super) fn simplify_expr_record(
        &mut self,
        expr: &mut stmt::ExprRecord,
    ) -> Option<stmt::Expr> {
        let mut all_values = true;

        for expr in &mut expr.fields {
            self.visit_expr_mut(expr);

            all_values &= expr.is_value();
        }

        if all_values {
            let mut values = vec![];

            for expr in expr.fields.drain(..) {
                let stmt::Expr::Value(value) = expr else {
                    panic!()
                };
                values.push(value);
            }

            Some(stmt::Value::record_from_vec(values).into())
        } else {
            None
        }
    }
}
