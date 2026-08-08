use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Rewrite `Intersects(path, [v1, v2, …])` into `OR(any_op(v1, Eq, path), …)`
    /// for drivers lacking `native_array_set_predicates`. The bottom-up fold
    /// pass has already collapsed a literal rhs into `Value::List` by the time
    /// this runs; a non-list rhs cannot be expanded and is left for the
    /// capability check to reject.
    pub(super) fn simplify_expr_intersects(
        &self,
        expr: &mut stmt::ExprIntersects,
    ) -> Option<stmt::Expr> {
        if self.capability.native_array_set_predicates {
            return None;
        }

        let stmt::Expr::Value(stmt::Value::List(values)) = expr.rhs.as_ref() else {
            return None;
        };

        // Empty-rhs has been folded to `false` upstream by `fold::expr_intersects`.
        debug_assert!(!values.is_empty());

        let path = (*expr.lhs).clone();
        let operands: Vec<stmt::Expr> = values
            .iter()
            .map(|v| stmt::Expr::any_op(v.clone(), stmt::BinaryOp::Eq, path.clone()))
            .collect();
        Some(stmt::Expr::or_from_vec(operands))
    }
}
