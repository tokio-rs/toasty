use toasty_core::stmt::{self, VisitMut};

use super::LowerStatement;

impl LowerStatement<'_, '_> {
    pub(super) fn lower_expr_begins_with(&mut self, expr: &mut stmt::Expr) {
        // SQL drivers don't have a native begins_with; rewrite to
        // `Like(expr, prefix || '%')`. The prefix is still a literal
        // Value at this point (extract_params hasn't run), so we can
        // append `%` directly instead of emitting a concat.
        let stmt::Expr::BeginsWith(mut e) = expr.take() else {
            panic!()
        };

        self.visit_expr_mut(&mut e.expr);
        self.visit_expr_mut(&mut e.prefix);

        let pattern = match *e.prefix {
            stmt::Expr::Value(stmt::Value::String(mut s)) => {
                s.push('%');
                stmt::Expr::Value(stmt::Value::String(s))
            }
            other => panic!("unexpected BeginsWith prefix expression: {other:?}"),
        };

        *expr = stmt::Expr::like(*e.expr, pattern);
    }
}
