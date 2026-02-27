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

        // Uniform arms: if every arm produces the same expression, the Match
        // is redundant — return that expression directly. This handles e.g.
        // Match(disc, [1 => disc, 2 => disc]) → disc
        if !expr.arms.is_empty() && expr.arms.iter().all(|arm| arm.expr == expr.arms[0].expr) {
            return Some(expr.arms[0].expr.clone());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::tests::test_schema;
    use toasty_core::stmt::{ExprMatch, MatchArm, Value};

    #[test]
    fn constant_subject_matches_arm() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `match I64(1) { 0 => "a", 1 => "b" }` → `"b"`
        let mut expr = ExprMatch {
            subject: Box::new(Expr::from(1i64)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("b"),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::String(ref s))) if s == "b"));
    }

    #[test]
    fn constant_subject_no_matching_arm_folds_to_else() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `match I64(99) { 0 => "a", 1 => "b" else => null }` — no arm matches, folds to else
        let mut expr = ExprMatch {
            subject: Box::new(Expr::from(99i64)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("b"),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);
        assert!(matches!(result, Some(Expr::Value(Value::Null))));
    }

    #[test]
    fn non_constant_subject_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // Non-constant subject → no simplification.
        let mut expr = ExprMatch {
            subject: Box::new(Expr::arg(0)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::from("a"),
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("b"),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn uniform_arms_folds_to_single_expr() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // Match(arg(0), [0 => arg(1), 1 => arg(1)]) → arg(1)
        let mut expr = ExprMatch {
            subject: Box::new(Expr::arg(0)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::arg(1),
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::arg(1),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);
        assert_eq!(result, Some(Expr::arg(1)));
    }

    #[test]
    fn non_uniform_arms_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // Match(arg(0), [0 => arg(1), 1 => arg(2)]) — different arm exprs, no fold.
        let mut expr = ExprMatch {
            subject: Box::new(Expr::arg(0)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::arg(1),
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::arg(2),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);
        assert!(result.is_none());
    }

    #[test]
    fn false_arm_expr_not_dropped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `arm.expr = false` is a return *value*, not a filter predicate.
        // The arm must be kept so that subj=0 returns `false`, not `null`.
        let mut expr = ExprMatch {
            subject: Box::new(Expr::arg(0)),
            arms: vec![
                MatchArm {
                    pattern: Value::from(0i64),
                    expr: Expr::FALSE,
                },
                MatchArm {
                    pattern: Value::from(1i64),
                    expr: Expr::from("b"),
                },
            ],
            else_expr: Box::new(Expr::null()),
        };

        let result = simplify.simplify_expr_match(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.arms.len(), 2, "false-valued arm must not be dropped");
    }
}
