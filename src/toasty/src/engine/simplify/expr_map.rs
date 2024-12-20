use super::*;

impl Simplify<'_> {
    pub(super) fn simplify_expr_map(&self, expr: &mut stmt::ExprMap) -> Option<Expr> {
        if let stmt::Expr::Value(value) = &mut *expr.base {
            todo!()
        } else {
            None
        }
    }
}
