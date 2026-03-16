use super::Simplify;
use toasty_core::stmt::{self, visit_mut};

impl Simplify<'_> {
    /// Inline a stable `Let` expression by substituting its bindings into the
    /// body.
    ///
    /// A `Let` is inlinable when every binding is *stable* — that is, fully
    /// resolved with no remaining `Stmt` nodes. After lowering converts
    /// `Stmt → Arg`, the bindings become stable and the Let can be removed by
    /// substituting each `Arg(position=i, nesting=0)` in the body with
    /// `bindings[i]`.
    pub(super) fn simplify_expr_let(&self, expr_let: &mut stmt::ExprLet) -> Option<stmt::Expr> {
        if !expr_let.bindings.iter().all(|b| b.is_stable()) {
            return None;
        }

        let mut body = *expr_let.body.clone();
        substitute_let_bindings(&mut body, &expr_let.bindings);
        Some(body)
    }
}

/// Substitute Let bindings into a body expression.
///
/// - `Arg(position=i, nesting==depth)` where `i < bindings.len()` is replaced
///   with `bindings[i].clone()`.
/// - `Arg(nesting > depth)` gets `nesting -= 1` (the Let scope is removed).
///
/// Uses `walk_expr_scoped_mut` to automatically track scope depth through
/// Let/Map scopes.
fn substitute_let_bindings(expr: &mut stmt::Expr, bindings: &[stmt::Expr]) {
    visit_mut::walk_expr_scoped_mut(expr, 0, |expr, scope_depth| match expr {
        stmt::Expr::Arg(arg) if arg.nesting == scope_depth && arg.position < bindings.len() => {
            *expr = bindings[arg.position].clone();
            false
        }
        stmt::Expr::Arg(arg) if arg.nesting > scope_depth => {
            arg.nesting -= 1;
            false
        }
        _ => true,
    });
}
