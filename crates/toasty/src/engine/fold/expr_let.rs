use toasty_core::stmt::{self, Expr, visit_mut};

/// Cheap canonicalization for `Let`: inline a `Let` whose bindings are all
/// stable by substituting each `Arg(position=i, nesting=0)` in the body
/// with `bindings[i]`.
///
/// A binding is *stable* when it is fully resolved with no remaining
/// `Stmt` nodes.  After lowering converts `Stmt → Arg`, the bindings
/// become stable and the `Let` collapses; the same shape arises at exec
/// time once bind-value substitution has replaced `Arg` with `Value`.
///
/// Schema-free and idempotent: a `Let` whose bindings are not all stable
/// is left alone, and inlining produces a body that contains no
/// references to the now-removed `Let` scope.
pub(super) fn fold_expr_let(expr_let: &mut stmt::ExprLet) -> Option<Expr> {
    if !expr_let.bindings.iter().all(|b| b.is_stable()) {
        return None;
    }

    let mut body = *expr_let.body.clone();
    substitute_let_bindings(&mut body, &expr_let.bindings);
    Some(body)
}

/// Substitute `Let` bindings into a body expression.
///
/// - `Arg(position=i, nesting==depth)` where `i < bindings.len()` is replaced
///   with `bindings[i].clone()`.
/// - `Arg(nesting > depth)` gets `nesting -= 1` (the `Let` scope is removed).
///
/// Uses `walk_expr_scoped_mut` to track scope depth through `Let`/`Map`
/// scopes automatically.
fn substitute_let_bindings(expr: &mut Expr, bindings: &[Expr]) {
    visit_mut::walk_expr_scoped_mut(expr, 0, |expr, scope_depth| match expr {
        Expr::Arg(arg) if arg.nesting == scope_depth && arg.position < bindings.len() => {
            *expr = bindings[arg.position].clone();
            false
        }
        Expr::Arg(arg) if arg.nesting > scope_depth => {
            arg.nesting -= 1;
            false
        }
        _ => true,
    });
}
