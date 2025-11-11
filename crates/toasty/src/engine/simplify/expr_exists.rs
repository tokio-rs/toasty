use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_exists(
        &self,
        expr_exists: &stmt::ExprExists,
    ) -> Option<stmt::Expr> {
        // EXISTS (empty query) -> false
        // NOT EXISTS (empty query) -> true
        if self.stmt_query_is_empty(&expr_exists.subquery) {
            return Some(stmt::Expr::from(expr_exists.negated));
        }

        None
    }
}
