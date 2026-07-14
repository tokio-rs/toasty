use toasty_core::stmt::{self, Expr, Value};

/// Cheap canonicalization for `is_superset(lhs, rhs)`: an empty rhs makes
/// the predicate vacuously true (every collection is a superset of the
/// empty set), regardless of `lhs`. The rhs is the user-supplied set
/// literal — `tags.is_superset(values)` — so an empty list here means
/// the caller passed `[]`.
pub(super) fn fold_expr_is_superset(expr: &mut stmt::ExprIsSuperset) -> Option<Expr> {
    if expr.rhs.is_list_empty() {
        return Some(Expr::Value(Value::Bool(true)));
    }
    None
}
