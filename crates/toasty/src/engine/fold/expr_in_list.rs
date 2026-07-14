use toasty_core::stmt::{self, Expr, Value, ValueSet};

/// Cheap canonicalization for `x IN (..)`: empty list → false, null
/// propagation, literal dedup, single-item collapse.
///
/// Heavyweight rewrites (model-reference → primary-key-reference) live in
/// `simplify/expr_in_list.rs` and run after this fold pass on canonical
/// input.
pub(super) fn fold_expr_in_list(expr: &mut stmt::ExprInList) -> Option<Expr> {
    // `x in ()` → `false`
    if expr.list.is_list_empty() {
        return Some(Expr::Value(Value::Bool(false)));
    }

    // Null propagation, `null in (x, y, z)` → `null`
    if expr.expr.is_value_null() {
        return Some(Expr::null());
    }

    // Deduplicate literal value lists: `x in (1, 1, 2)` → `x in (1, 2)`.
    // Lists carry literal `Value`s only (no symbolic expressions), so value
    // equality is safe — there is no non-determinism to preserve.
    if let Expr::Value(Value::List(values)) = &mut *expr.list {
        let mut seen = ValueSet::new();
        values.retain(|v| seen.insert(v.clone()));
    }

    // Rewrite single-item lists into equalities.
    rewrite_expr_in_list_with_single_item(expr)
}

fn rewrite_expr_in_list_with_single_item(expr: &mut stmt::ExprInList) -> Option<Expr> {
    let rhs = match &mut *expr.list {
        Expr::Value(value) => {
            let values = match value {
                Value::List(value) => &value[..],
                _ => todo!("{value:#?}"),
            };

            if values.len() != 1 {
                return None;
            }

            Expr::Value(values[0].clone())
        }
        Expr::List(expr_list) => {
            if expr_list.items.len() != 1 {
                return None;
            }

            expr_list.items[0].take()
        }
        Expr::Record(_) => todo!("should not happen"),
        _ => return None,
    };

    Some(Expr::eq(expr.expr.take(), rhs))
}
