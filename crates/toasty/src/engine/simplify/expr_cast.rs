use super::Simplify;
use toasty_core::schema::app::FieldTy;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_cast(&self, expr: &mut stmt::ExprCast) -> Option<stmt::Expr> {
        // Constant folding: evaluate the cast at compile time.
        if let stmt::Expr::Value(value) = &mut *expr.expr {
            let cast = expr.ty.cast(value.take()).unwrap();
            return Some(cast.into());
        }

        // Redundant cast elimination: `cast(x as T) → x` when x is already T.
        //
        // If the inner expression is a field reference whose primitive type
        // matches the cast target, the cast is a no-op and can be removed.
        if let stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) = &*expr.expr {
            let field = self.cx.resolve_expr_reference(f).as_field_unwrap();
            if let FieldTy::Primitive(primitive) = &field.ty
                && primitive.ty == expr.ty
            {
                return Some(expr.expr.take());
            }
        }

        None
    }
}
