use super::*;

use stmt::{Expr, Value};

impl SimplifyExpr<'_> {
    pub(super) fn simplify_expr_in_list(&self, expr: &mut stmt::ExprInList) -> Option<Expr> {
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
            Expr::Record(expr_record) => {
                if expr_record.len() != 1 {
                    return None;
                }

                expr_record[0].take()
            }
            _ => return None,
        };

        Some(Expr::eq(expr.expr.take(), rhs))
    }
}
