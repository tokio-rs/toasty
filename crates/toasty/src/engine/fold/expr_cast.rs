use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `CAST`: constant-fold the cast at compile
/// time when the operand is a literal value.
///
/// Heavyweight rewrites (redundant-cast elimination on field references,
/// which needs schema lookup for the field's type) live in
/// `simplify/expr_cast.rs` and run after this fold pass on canonical
/// input.
pub(super) fn fold_expr_cast(expr: &mut stmt::ExprCast) -> Option<Expr> {
    let stmt::Expr::Value(value) = &mut *expr.expr else {
        return None;
    };

    let cast = expr.ty.cast(value.take()).unwrap();
    Some(cast.into())
}
