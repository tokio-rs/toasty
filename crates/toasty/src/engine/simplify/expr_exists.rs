use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_exists(
        &self,
        expr_exists: &stmt::ExprExists,
    ) -> Option<stmt::Expr> {
        // `exists(empty_query)` → `false`
        if self.stmt_query_is_empty(&expr_exists.subquery) {
            return Some(stmt::Expr::FALSE);
        }

        None
    }
}
