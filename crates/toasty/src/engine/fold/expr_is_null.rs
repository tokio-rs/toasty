use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `IS NULL`: constant folding, cast stripping.
///
/// Heavyweight rewrites (non-nullable field → false) live in
/// `simplify/expr_is_null.rs` and run after this fold pass on canonical
/// input.
pub(super) fn fold_expr_is_null(expr: &mut stmt::ExprIsNull) -> Option<Expr> {
    match &mut *expr.expr {
        // Null constant folding:
        //  - `null is null` → `true`
        //  - `<non-null const> is null` → `false`
        stmt::Expr::Value(value) => Some(value.is_null().into()),
        // A constructed record or list is structurally non-null, so
        // `is_null(record)` → `false`. This fires when an `Option<Embed>`
        // encode guard `is_not_null(field)` sees a `Some` value: the embed
        // lowers to an `Expr::Record` of its columns, so the guard subject
        // folds to `is_not_null(Record(..))` → `not(false)` → `true`.
        stmt::Expr::Record(_) | stmt::Expr::List(_) => Some(false.into()),
        // Strip type casts: `is_null(cast(x, T))` → `is_null(x)`.
        // Nullity is type-independent so the cast is unnecessary.
        stmt::Expr::Cast(expr_cast) => {
            *expr.expr = expr_cast.expr.take();
            None
        }
        _ => None,
    }
}
