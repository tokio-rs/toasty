use super::*;

use crate::engine::eval2::Eval;

impl Simplify<'_> {
    pub(super) fn simplify_expr_map(&self, expr: &mut stmt::Expr) -> Option<Expr> {
        if expr.as_map().base.is_value() {
            let Some(eval) = Eval::try_from_stmt(vec![], expr) else {
                return None;
            };

            let ret = eval.eval(&[]).unwrap();
            Some(stmt::Expr::Value(ret))
        } else {
            None
        }
    }
}
