use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_any(&self, expr_any: &stmt::ExprAny) -> Option<Expr> {
        // Only simplify when the inner expression is a Map with a constant base
        let stmt::Expr::Map(expr_map) = &*expr_any.expr else {
            return None;
        };

        if !expr_map.base.is_const() {
            return None;
        }

        let Ok(base) = expr_map.base.eval_const() else {
            return None;
        };
        let stmt::Value::List(items) = base else {
            todo!()
        };

        let mut operands = vec![];

        for item in items {
            let mut operand = (*expr_map.map).clone();

            match item {
                stmt::Value::Record(value_record) => operand.substitute(&value_record.fields[..]),
                item => operand.substitute([item]),
            }

            operands.push(operand);
        }

        Some(Expr::or_from_vec(operands))
    }
}
