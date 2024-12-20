use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_expr_is_null(&self, expr: &mut stmt::ExprIsNull) -> Option<stmt::Expr> {
        match &mut *expr.expr {
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr.expr = expr_cast.expr.take();
                None
            }
            stmt::Expr::Value(_) => todo!("expr={expr:#?}"),
            _ => None,
        }
    }
}
