use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `CAST`: constant-fold the cast at compile
/// time when the operand is a literal value.
///
/// Schema-directed casts — a `#[document]` embed's record ↔ object
/// conversions, marked by a source type or a model-typed target — are
/// skipped: fold is schema-free, so they constant-fold in the simplifier
/// instead (`simplify/expr_cast.rs`), which also owns the other heavyweight
/// rewrites (redundant-cast elimination on field references).
pub(super) fn fold_expr_cast(expr: &mut stmt::ExprCast) -> Option<Expr> {
    // `DEFAULT` is a SQL sentinel rather than a typed value. A storage bridge
    // may wrap it when an auto-increment column's database type differs from
    // its application type, but the cast must not reach SQL serialization.
    if matches!(*expr.expr, stmt::Expr::Default) {
        return Some(stmt::Expr::Default);
    }

    if expr.from.is_some() || expr.ty.contains_model() {
        return None;
    }

    let stmt::Expr::Value(value) = &mut *expr.expr else {
        return None;
    };

    let cast = expr.ty.cast(&(), value.take()).unwrap();
    Some(cast.into())
}
