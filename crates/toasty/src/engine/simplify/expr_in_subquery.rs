use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_in_subquery(
        &self,
        expr: &stmt::ExprInSubquery,
    ) -> Option<stmt::Expr> {
        // A `WITH` clause may carry side effects that must still run.
        if expr.query.with.is_some() {
            return None;
        }

        // `x in (empty_query)` → `false`, `x not in (empty_query)` → `true`
        if self.stmt_query_is_empty(&expr.query) {
            return Some(stmt::Expr::from(expr.negated));
        }

        None
    }
}
