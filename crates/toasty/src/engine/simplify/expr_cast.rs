use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_expr_cast(&self, expr: &mut stmt::ExprCast) -> Option<stmt::Expr> {
        if let stmt::Expr::Value(value) = &mut *expr.expr {
            let cast = expr.ty.cast(value.take()).unwrap();
            Some(cast.into())
        } else {
            None
        }
    }
}
