use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_match(&mut self, expr: &mut stmt::ExprMatch) -> Option<stmt::Expr> {
        // Constant subject folding: if the subject is a constant value, find the
        // matching arm and return its expression.
        if let Expr::Value(ref value) = *expr.subject {
            for arm in &expr.arms {
                if value == &arm.pattern {
                    return Some(arm.expr.clone());
                }
            }
            return Some(*expr.else_expr.clone());
        }

        // Uniform arms: if every arm AND the else branch produce the same
        // expression, the Match is redundant — return that expression directly.
        // This handles e.g. Match(disc, [1 => disc, 2 => disc], else: disc) → disc
        if !expr.arms.is_empty()
            && expr.arms.iter().all(|arm| arm.expr == expr.arms[0].expr)
            && *expr.else_expr == expr.arms[0].expr
        {
            return Some(expr.arms[0].expr.clone());
        }

        None
    }
}
