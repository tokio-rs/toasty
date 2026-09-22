use toasty_core::stmt;

/// Wrap a loaded value to distinguish loaded NULL from an unloaded field.
pub(crate) fn loaded_expr(value: stmt::Expr) -> stmt::Expr {
    stmt::Expr::record([value])
}
