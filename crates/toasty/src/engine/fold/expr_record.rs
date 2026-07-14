use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `Record`: when every field is a literal
/// `Value`, collapse the record to a `Value::Record`.
///
/// All `Record` rules are local and schema-free; there is no heavyweight
/// counterpart in `simplify`.
pub(super) fn fold_expr_record(expr: &mut stmt::ExprRecord) -> Option<Expr> {
    let values = expr
        .fields
        .iter()
        .map(|field| match field {
            stmt::Expr::Value(value) => Some(value.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    Some(stmt::Value::record_from_vec(values).into())
}
