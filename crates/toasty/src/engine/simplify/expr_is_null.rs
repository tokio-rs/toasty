use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Heavyweight `IS NULL` rewrites. Cheap canonicalization (constant
    /// folding, cast stripping) runs in `fold::expr_is_null` before this is
    /// reached.
    pub(super) fn simplify_expr_is_null(&self, expr: &mut stmt::ExprIsNull) -> Option<stmt::Expr> {
        let stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) = &mut *expr.expr else {
            return None;
        };

        let field = self.cx.resolve_expr_reference(f).as_field_unwrap();

        if !field.nullable() {
            // `is_null` on a non-nullable field evaluates to `false`.
            return Some(stmt::Expr::Value(stmt::Value::Bool(false)));
        }

        None
    }
}
