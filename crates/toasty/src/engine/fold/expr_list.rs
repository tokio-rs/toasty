use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `List`: when every item is a literal
/// `Value`, collapse the list to a `Value::List`.
///
/// Heavyweight rewrites (merging single-row Insert statements into a
/// batch Insert) live in `simplify/expr_list.rs` and run after this
/// fold pass on canonical input.
pub(super) fn fold_expr_list(expr: &mut stmt::ExprList) -> Option<Expr> {
    let values = expr
        .items
        .iter()
        .map(|item| match item {
            stmt::Expr::Value(value) => Some(value.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    Some(stmt::Value::list_from_vec(values).into())
}
