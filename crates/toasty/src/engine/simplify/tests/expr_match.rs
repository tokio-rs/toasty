use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{self, Expr, ExprMatch, MatchArm, Projection, Value, VisitMut};

// --- simplify_expr_match unit tests ---

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
fn uniform_arms_and_else_folds_to_single_expr() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // Match(arg(0), [0 => arg(1), 1 => arg(1)], else: arg(1)) → arg(1)
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

    let result = simplify.simplify_expr_match(&mut expr);
    assert_eq!(result, Some(Expr::arg(1)));
}

#[test]
fn uniform_arms_but_different_else_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // Match(arg(0), [0 => arg(1), 1 => arg(1)], else: null) — arms match but
    // else differs, so the Match cannot be folded away.
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
    assert!(result.is_none());
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

// --- visit_expr_mut end-to-end tests ---

/// `visit_expr_mut` on a `Match` with a constant subject folds the whole
/// expression to the matching arm's value (end-to-end through the override).
#[test]
fn constant_subject_folds_to_arm_value() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `match I64(2) { 1 => "a", 2 => "b", 3 => "c" }` → `"b"`
    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::from(2i64)),
        arms: vec![
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::from("a"),
            },
            MatchArm {
                pattern: Value::from(2i64),
                expr: Expr::from("b"),
            },
            MatchArm {
                pattern: Value::from(3i64),
                expr: Expr::from("c"),
            },
        ],
        else_expr: Box::new(Expr::null()),
    });

    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Value(Value::String(s)) if s == "b"));
}

/// The subject expression is simplified before the fold decision is made.
/// A `project([0], record([I64(1)]))` subject should first simplify to
/// `I64(1)` and then the matching arm should be selected.
#[test]
fn subject_simplified_before_folding() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // subject = `project([0], record([I64(1)]))` which simplifies to `I64(1)`
    let subject = stmt::ExprProject {
        base: Box::new(Expr::record([Expr::from(1i64)])),
        projection: Projection::from(0),
    };

    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::Project(subject)),
        arms: vec![
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::from("matched"),
            },
            MatchArm {
                pattern: Value::from(2i64),
                expr: Expr::from("other"),
            },
        ],
        else_expr: Box::new(Expr::null()),
    });

    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Value(Value::String(s)) if s == "matched"));
}

/// Dead-code arms are NOT visited when the subject is constant. This is the
/// critical bug prevention: a dead arm containing an invalid projection such as
/// `project([1], record([I64(1)]))` (index out of bounds) must not be
/// simplified, otherwise the simplifier would panic.
#[test]
fn dead_arms_not_visited_with_constant_subject() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // Arm 2 (`project([1], record([I64(1)]))`) would panic if simplified
    // because the record only has 1 element. Since subject is I64(1) → arm 1
    // is selected, arm 2 must be skipped entirely.
    let dead_arm_expr = stmt::ExprProject {
        base: Box::new(Expr::record([Expr::from(1i64)])),
        projection: Projection::from(1), // index 1 into a 1-element record → OOB
    };

    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::from(1i64)),
        arms: vec![
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::from("ok"),
            },
            MatchArm {
                pattern: Value::from(2i64),
                expr: Expr::Project(dead_arm_expr),
            },
        ],
        else_expr: Box::new(Expr::null()),
    });

    // Must not panic.
    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Value(Value::String(s)) if s == "ok"));
}

/// With a non-constant subject, all arms are simplified normally.
#[test]
fn non_constant_subject_simplifies_all_arms() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // Arms contain simplifiable sub-expressions (`record([x]) → {x}` when x
    // is constant). With a non-constant subject, both arms should be visited.
    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::arg(0)),
        arms: vec![
            MatchArm {
                pattern: Value::from(1i64),
                // `record(["hello"])` → constant record value after simplification
                expr: Expr::record([Expr::from("hello")]),
            },
            MatchArm {
                pattern: Value::from(2i64),
                expr: Expr::record([Expr::from("world")]),
            },
        ],
        else_expr: Box::new(Expr::null()),
    });

    simplify.visit_expr_mut(&mut expr);

    // Match is not folded (non-constant subject), but the arm exprs are simplified.
    let Expr::Match(m) = &expr else {
        panic!("expected Expr::Match")
    };
    assert!(matches!(&m.arms[0].expr, Expr::Value(Value::Record(_))));
    assert!(matches!(&m.arms[1].expr, Expr::Value(Value::Record(_))));
}

/// When the subject is constant and no arm matches, the match folds to the
/// else expression — even if that else expression is `Expr::Error`.
#[test]
fn constant_subject_no_match_folds_to_error_else() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `match 99 { 1 => "a" } else error("unexpected")` → `error("unexpected")`
    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::from(99i64)),
        arms: vec![MatchArm {
            pattern: Value::from(1i64),
            expr: Expr::from("a"),
        }],
        else_expr: Box::new(Expr::error("unexpected")),
    });

    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Error(e) if e.message == "unexpected"));
}

/// When the subject is constant and the matching arm body is `Expr::Error`,
/// the match folds to that error.
#[test]
fn constant_subject_matching_arm_is_error() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `match 1 { 1 => error("bad"), 2 => "ok" } else "default"` → `error("bad")`
    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::from(1i64)),
        arms: vec![
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::error("bad"),
            },
            MatchArm {
                pattern: Value::from(2i64),
                expr: Expr::from("ok"),
            },
        ],
        else_expr: Box::new(Expr::from("default")),
    });

    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Error(e) if e.message == "bad"));
}

/// When the subject is constant and a normal arm matches, the error else
/// branch is not reached.
#[test]
fn constant_subject_match_found_error_else_not_reached() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `match 1 { 1 => "ok" } else error("unexpected")` → `"ok"`
    let mut expr = Expr::Match(ExprMatch {
        subject: Box::new(Expr::from(1i64)),
        arms: vec![MatchArm {
            pattern: Value::from(1i64),
            expr: Expr::from("ok"),
        }],
        else_expr: Box::new(Expr::error("unexpected")),
    });

    simplify.visit_expr_mut(&mut expr);

    assert!(matches!(&expr, Expr::Value(Value::String(s)) if s == "ok"));
}
