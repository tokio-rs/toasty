use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_expr_concat_str(
        &self,
        expr: &mut stmt::ExprConcatStr,
    ) -> Option<stmt::Expr> {
        if expr.exprs.iter().all(|expr| expr.is_value()) {
            let mut ret = String::new();

            for expr in &expr.exprs {
                let stmt::Expr::Value(value) = expr else {
                    todo!()
                };

                match value {
                    stmt::Value::String(s) => ret.push_str(s),
                    _ => todo!("value={value:#?}"),
                }
            }

            Some(stmt::Expr::Value(ret.into()))
        } else {
            None
        }
    }
}
