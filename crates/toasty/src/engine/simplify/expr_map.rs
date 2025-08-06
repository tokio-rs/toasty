use super::Simplify;
use crate::engine::eval::Func;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_map(&self, expr: &mut stmt::Expr) -> Option<Expr> {
        if expr.as_map().base.is_value() {
            let eval = Func::try_from_stmt(expr, vec![])?;

            let ret = eval.eval(&[]).unwrap();
            Some(stmt::Expr::Value(ret))
        } else {
            None
        }
    }
}
