use toasty_core::stmt::{self, Expr, Value};

/// Cheap canonicalization for `intersects(lhs, rhs)`: an empty rhs makes
/// the predicate vacuously false (no collection intersects the empty
/// set), regardless of `lhs`. The rhs is the user-supplied set literal —
/// `tags.intersects(values)` — so an empty list here means the caller
/// passed `[]`.
pub(super) fn fold_expr_intersects(expr: &mut stmt::ExprIntersects) -> Option<Expr> {
    if expr.rhs.is_list_empty() {
        return Some(Expr::Value(Value::Bool(false)));
    }
    None
}
