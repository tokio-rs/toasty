use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_is_variant(
        &self,
        _expr: &mut stmt::ExprIsVariant,
    ) -> Option<stmt::Expr> {
        // Future: OR tautology elimination
        // (is_a() || is_b() over {A, B} → TRUE)
        None
    }
}
