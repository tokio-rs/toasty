use super::Simplify;
use toasty_core::stmt;

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
        substitute_let_bindings(&mut body, &expr_let.bindings, 0);
        Some(body)
    }
}

/// Substitute Let bindings into a body expression at a given nesting depth.
///
/// - `Arg(position=i, nesting==depth)` where `i < bindings.len()` is replaced
///   with `bindings[i].clone()`.
/// - `Arg(nesting > depth)` gets `nesting -= 1` (the Let scope is removed).
/// - Descending into `Map` or `Let` bodies increments `depth` because those
///   introduce their own scope.
fn substitute_let_bindings(expr: &mut stmt::Expr, bindings: &[stmt::Expr], depth: usize) {
    match expr {
        stmt::Expr::Arg(arg) if arg.nesting == depth && arg.position < bindings.len() => {
            *expr = bindings[arg.position].clone();
        }
        stmt::Expr::Arg(arg) if arg.nesting > depth => {
            arg.nesting -= 1;
        }
        stmt::Expr::Map(map) => {
            substitute_let_bindings(&mut map.base, bindings, depth);
            substitute_let_bindings(&mut map.map, bindings, depth + 1);
        }
        stmt::Expr::Let(inner_let) => {
            for binding in &mut inner_let.bindings {
                substitute_let_bindings(binding, bindings, depth);
            }
            substitute_let_bindings(&mut inner_let.body, bindings, depth + 1);
        }
        // Recurse into children for all other compound expressions
        stmt::Expr::And(e) => {
            for op in e.operands.iter_mut() {
                substitute_let_bindings(op, bindings, depth);
            }
        }
        stmt::Expr::Any(e) => substitute_let_bindings(&mut e.expr, bindings, depth),
        stmt::Expr::BinaryOp(e) => {
            substitute_let_bindings(&mut e.lhs, bindings, depth);
            substitute_let_bindings(&mut e.rhs, bindings, depth);
        }
        stmt::Expr::Cast(e) => substitute_let_bindings(&mut e.expr, bindings, depth),
        stmt::Expr::IsNull(e) => substitute_let_bindings(&mut e.expr, bindings, depth),
        stmt::Expr::IsVariant(e) => substitute_let_bindings(&mut e.expr, bindings, depth),
        stmt::Expr::Not(e) => substitute_let_bindings(&mut e.expr, bindings, depth),
        stmt::Expr::Or(e) => {
            for op in e.operands.iter_mut() {
                substitute_let_bindings(op, bindings, depth);
            }
        }
        stmt::Expr::List(e) => {
            for item in e.items.iter_mut() {
                substitute_let_bindings(item, bindings, depth);
            }
        }
        stmt::Expr::Record(e) => {
            for field in e.fields.iter_mut() {
                substitute_let_bindings(field, bindings, depth);
            }
        }
        stmt::Expr::Project(e) => substitute_let_bindings(&mut e.base, bindings, depth),
        stmt::Expr::InList(e) => {
            substitute_let_bindings(&mut e.expr, bindings, depth);
            substitute_let_bindings(&mut e.list, bindings, depth);
        }
        stmt::Expr::Match(e) => {
            substitute_let_bindings(&mut e.subject, bindings, depth);
            for arm in e.arms.iter_mut() {
                substitute_let_bindings(&mut arm.expr, bindings, depth);
            }
            substitute_let_bindings(&mut e.else_expr, bindings, depth);
        }
        // Leaf nodes — nothing to recurse into
        stmt::Expr::Arg(_)
        | stmt::Expr::Value(_)
        | stmt::Expr::Default
        | stmt::Expr::Error(_)
        | stmt::Expr::Reference(_)
        | stmt::Expr::Func(_)
        | stmt::Expr::Stmt(_)
        | stmt::Expr::Exists(_)
        | stmt::Expr::InSubquery(_) => {}
    }
}
