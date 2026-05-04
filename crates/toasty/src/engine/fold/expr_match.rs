use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for `Match`: constant-subject arm selection and
/// uniform-arms collapse. Both rules are local and schema-free.
///
/// Heavyweight `Match` rewrites (e.g., distributing projections into arms) live
/// in [`simplify::Simplify::simplify_expr_match`] (none currently).
///
/// Note: `simplify::Simplify` overrides `visit_expr_match_mut` to skip
/// dead-code arms when the subject is constant. That override prevents
/// downstream rules (notably `simplify_expr_project`) from panicking on
/// invalid sub-expressions in unreachable arms. Fold has no such override
/// because it has no rules that can panic on invalid input — every fold rule
/// matches structurally and bails to `None` on unexpected shape.
pub(super) fn fold_expr_match(expr: &mut stmt::ExprMatch) -> Option<Expr> {
    // Constant-subject arm selection,
    //
    //   - `match v { p1 => e1, ..., pN => eN } else => e0` with `v` constant
    //     and `v == pI` → `eI`
    //   - `match v { p1 => e1, ..., pN => eN } else => e0` with `v` constant
    //     and no `pI == v` → `e0`
    if let Expr::Value(value) = expr.subject.as_ref() {
        for arm in &expr.arms {
            if value == &arm.pattern {
                return Some(arm.expr.clone());
            }
        }
        return Some(expr.else_expr.as_ref().clone());
    }

    // Uniform arms,
    //
    //   - `match v { p1 => e, ..., pN => e } else => e` → `e`
    //
    // When every arm body and the else branch produce the same expression,
    // the match is independent of the subject and reduces to that expression.
    let first = expr.arms.first()?;
    let all_arms_match = expr.arms.iter().all(|arm| arm.expr == first.expr);
    let else_matches = expr.else_expr.as_ref() == &first.expr;
    match () {
        () if all_arms_match && else_matches => Some(first.expr.clone()),
        () => None,
    }
}
