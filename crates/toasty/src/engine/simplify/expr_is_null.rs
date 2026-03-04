use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_is_null(&self, expr: &mut stmt::ExprIsNull) -> Option<stmt::Expr> {
        match &mut *expr.expr {
            stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) => {
                let field = self.cx.resolve_expr_reference(f).expect_field();

                if !field.nullable() {
                    // Is null on a non nullable field evaluates to `false`.
                    return Some(stmt::Expr::Value(stmt::Value::Bool(false)));
                }

                None
            }
            // Null constant folding,
            //
            //  - `null is null` → `true`
            //  - `<non-null const> is null` → `false`
            stmt::Expr::Value(value) => Some(value.is_null().into()),
            _ => None,
        }
    }
}
