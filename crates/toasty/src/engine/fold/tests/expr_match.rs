use crate::engine::fold::expr_match::fold_expr_match;
use toasty_core::stmt::{Expr, ExprMatch, MatchArm, Value};

#[test]
fn constant_subject_matches_arm() {
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

    let result = fold_expr_match(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::String(ref s))) if s == "b"));
}

#[test]
fn constant_subject_no_matching_arm_folds_to_else() {
    // `match I64(99) { 0 => "a", 1 => "b" else => null }` — no arm matches,
    // folds to else
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

    let result = fold_expr_match(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

#[test]
fn non_constant_subject_not_simplified() {
    // Non-constant subject → no fold.
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

    let result = fold_expr_match(&mut expr);

    assert!(result.is_none());
}

#[test]
fn uniform_arms_and_else_folds_to_single_expr() {
    // `match arg(0) { 0 => arg(1), 1 => arg(1) } else => arg(1)` → `arg(1)`
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
        else_expr: Box::new(Expr::arg(1)),
    };

    let result = fold_expr_match(&mut expr);

    assert_eq!(result, Some(Expr::arg(1)));
}

#[test]
fn uniform_arms_but_different_else_not_simplified() {
    // `match arg(0) { 0 => arg(1), 1 => arg(1) } else => null` — arms match
    // but else differs, so the match cannot be folded away.
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

    let result = fold_expr_match(&mut expr);

    assert!(result.is_none());
}

#[test]
fn non_uniform_arms_not_simplified() {
    // `match arg(0) { 0 => arg(1), 1 => arg(2) }` — different arm exprs,
    // no fold.
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

    let result = fold_expr_match(&mut expr);

    assert!(result.is_none());
}

#[test]
fn empty_arms_not_simplified() {
    // No arms → cannot apply uniform-arms rule (no first arm to compare to).
    let mut expr = ExprMatch {
        subject: Box::new(Expr::arg(0)),
        arms: vec![],
        else_expr: Box::new(Expr::null()),
    };

    let result = fold_expr_match(&mut expr);

    assert!(result.is_none());
}

#[test]
fn false_arm_expr_not_dropped() {
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

    let result = fold_expr_match(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.arms.len(), 2, "false-valued arm must not be dropped");
}
