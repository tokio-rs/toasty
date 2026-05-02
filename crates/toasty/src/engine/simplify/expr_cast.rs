use super::Simplify;
use toasty_core::schema::app::FieldTy;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Heavyweight `CAST` rewrites. Cheap canonicalization (constant
    /// folding) runs in `fold::expr_cast` before this is reached.
    pub(super) fn simplify_expr_cast(&self, expr: &mut stmt::ExprCast) -> Option<stmt::Expr> {
        // Redundant cast elimination: `cast(x as T) → x` when x is already T.
        //
        // If the inner expression is a field reference whose primitive type
        // matches the cast target, the cast is a no-op and can be removed.
        let stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) = &*expr.expr else {
            return None;
        };

        let field = self.cx.resolve_expr_reference(f).as_field_unwrap();
        let FieldTy::Primitive(primitive) = &field.ty else {
            return None;
        };

        if primitive.ty == expr.ty {
            return Some(expr.expr.take());
        }

        None
    }
}
