//! Cheap, schema-free, idempotent rewrites.
//!
//! `fold` is the canonicalization pass shared between `lower`, `simplify`, and
//! `exec_statement`. Each rule is local, O(n) per pass, and produces output
//! that satisfies `fold(fold(x)) == fold(x)`. Rules pattern-match on local
//! structure only; no schema lookups.
//!
//! See `docs/dev/design/lower-then-simplify.md` for the full taxonomy.

mod expr_and;
mod expr_binary_op;
mod expr_cast;
mod expr_in_list;
mod expr_is_null;
mod expr_list;
mod expr_match;
mod expr_not;
mod expr_or;
mod expr_record;

#[cfg(test)]
mod tests;

use toasty_core::stmt::{self, Expr, Node, VisitMut};

/// Folds an expression tree in place: canonicalizes structure, evaluates
/// constants, propagates nulls, drops boolean units. Idempotent.
pub(crate) fn fold_stmt<T: Node>(stmt: &mut T) {
    Fold.visit_mut(stmt);
}

/// Visitor that applies fold rules bottom-up.
struct Fold;

impl VisitMut for Fold {
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        // Recurse into children first so parent rules see canonical input.
        stmt::visit_mut::visit_expr_mut(self, i);

        // A single rule's output may itself be fold-eligible (e.g.
        // `not(eq(x, y))` becomes `ne(x, y)`, which then constant-folds
        // to a literal when both sides are values, or De Morgan creates a
        // new `Or` of `Not`s that need their own pass). Recurse on the
        // replacement so the fixed point is reached in one traversal.
        if let Some(mut expr) = fold_one(i) {
            self.visit_expr_mut(&mut expr);
            *i = expr;
        }
    }
}

/// Apply one round of fold rules to `i`. Returns `Some` if a rule fired and
/// produced a replacement expression; returns `None` if no rule applied or
/// the rule mutated `i` in place.
fn fold_one(i: &mut Expr) -> Option<Expr> {
    match i {
        Expr::And(expr) => expr_and::fold_expr_and(expr),
        Expr::BinaryOp(expr) => {
            expr_binary_op::fold_expr_binary_op(expr.op, &mut expr.lhs, &mut expr.rhs)
        }
        Expr::Cast(expr) => expr_cast::fold_expr_cast(expr),
        Expr::InList(expr) => expr_in_list::fold_expr_in_list(expr),
        Expr::IsNull(expr) => expr_is_null::fold_expr_is_null(expr),
        Expr::List(expr) => expr_list::fold_expr_list(expr),
        Expr::Match(expr) => expr_match::fold_expr_match(expr),
        Expr::Not(expr) => expr_not::fold_expr_not(expr),
        Expr::Or(expr) => expr_or::fold_expr_or(expr),
        Expr::Record(expr) => expr_record::fold_expr_record(expr),
        // No fold rules yet — leaves and not-yet-migrated variants pass
        // through untouched. Listed explicitly so adding a new `Expr`
        // variant forces a decision about whether it has cheap rewrites.
        Expr::AllOp(_)
        | Expr::Any(_)
        | Expr::AnyOp(_)
        | Expr::Arg(_)
        | Expr::Default
        | Expr::Error(_)
        | Expr::Exists(_)
        | Expr::Func(_)
        | Expr::Ident(_)
        | Expr::InSubquery(_)
        | Expr::Intersects(_)
        | Expr::IsSuperset(_)
        | Expr::IsVariant(_)
        | Expr::Length(_)
        | Expr::Let(_)
        | Expr::Like(_)
        | Expr::Map(_)
        | Expr::Project(_)
        | Expr::Reference(_)
        | Expr::StartsWith(_)
        | Expr::Stmt(_)
        | Expr::Value(_) => None,
    }
}
