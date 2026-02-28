use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{BinaryOp, Expr, ExprAnd, ExprOr};

/// Builds `and(a, and(b, c))`, a nested AND structure for testing
/// flattening.
fn nested_and(a: Expr, b: Expr, c: Expr) -> ExprAnd {
    ExprAnd {
        operands: vec![
            a,
            Expr::And(ExprAnd {
                operands: vec![b, c],
            }),
        ],
    }
}

#[test]
fn flatten_all_symbolic() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(A, and(B, C)) → and(A, B, C)`
    let mut expr = nested_and(Expr::arg(0), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none()); // Modified in place
    assert_eq!(expr.operands.len(), 3);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
    assert_eq!(expr.operands[2], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_outer() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(true, and(B, C)) → and(B, C)`
    let mut expr = nested_and(true.into(), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(1));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_nested_first() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(A, and(true, C)) → and(A, C)`
    let mut expr = nested_and(Expr::arg(0), true.into(), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_nested_second() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(A, and(B, true)) → and(A, B)`
    let mut expr = nested_and(Expr::arg(0), Expr::arg(1), true.into());
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn flatten_outer_true_nested_one_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(true, and(true, C)) → C`
    let mut expr = nested_and(true.into(), true.into(), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(2));
}

#[test]
fn flatten_outer_symbolic_nested_all_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(A, and(true, true)) → A`
    let mut expr = nested_and(Expr::arg(0), true.into(), true.into());
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn flatten_all_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(true, and(true, true)) → true`
    let mut expr = nested_and(true.into(), true.into(), true.into());
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn flatten_with_false_in_outer() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(false, and(B, C)) → false`
    let mut expr = nested_and(false.into(), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_with_false_in_nested() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(A, and(false, C)) → false`
    let mut expr = nested_and(Expr::arg(0), false.into(), Expr::arg(2));
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_true_and_false_mixed() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(true, and(false, true)) → false`
    let mut expr = nested_and(true.into(), false.into(), true.into());
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn single_operand_unwrapped() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(arg(0)) → arg(0)`
    let mut expr = ExprAnd {
        operands: vec![Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn empty_after_removing_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(true, true) → true`
    let mut expr = ExprAnd {
        operands: vec![true.into(), true.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn idempotent_two_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, a) → a`
    let mut expr = ExprAnd {
        operands: vec![Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_three_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, a, a) → a`
    let mut expr = ExprAnd {
        operands: vec![Expr::arg(0), Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_with_different() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, b, a) → and(a, b)`
    let mut expr = ExprAnd {
        operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_and_or() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, or(a, b))` → `a`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::arg(0),
            Expr::Or(ExprOr {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn absorption_with_multiple_operands() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, b, or(a, c))` → `and(a, b)`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::Or(ExprOr {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_two_and_three_or() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(a, b, or(a, c, d))` → `and(a, b)`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::Or(ExprOr {
                operands: vec![Expr::arg(0), Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn complement_basic() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a and not(a)` → `false` (where a is a non-nullable comparison)
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprAnd {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn complement_with_other_operands() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a and b and not(a)` → `false`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprAnd {
        operands: vec![
            a.clone(),
            Expr::arg(2),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn complement_nullable_not_simplified() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a and not(a)` where `a` is an arg (nullable) → no change
    let a = Expr::arg(0);
    let mut expr = ExprAnd {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
}

#[test]
fn complement_multiple_repetitions() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a and a and not(a) and not(a)` → `false`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprAnd {
        operands: vec![
            a.clone(),
            a.clone(),
            Expr::Not(ExprNot {
                expr: Box::new(a.clone()),
            }),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn range_to_equality_ge_le() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a >= 5 and a <= 5` → `a = 5`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    let Some(Expr::BinaryOp(bin_op)) = result else {
        panic!("expected binary op");
    };
    assert!(bin_op.op.is_eq());
}

#[test]
fn range_to_equality_le_ge() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a <= 5 and a >= 5` → `a = 5` (opposite order)
    let mut expr = ExprAnd {
        operands: vec![
            Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    let Some(Expr::BinaryOp(bin_op)) = result else {
        panic!("expected binary op");
    };
    assert!(bin_op.op.is_eq());
}

#[test]
fn range_to_equality_different_bounds_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a >= 5 and a <= 10` is not simplified (different bounds)
    let mut expr = ExprAnd {
        operands: vec![
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(0), BinaryOp::Le, 10i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn range_to_equality_different_exprs_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a >= 5 and b <= 5` is not simplified (different expressions)
    let mut expr = ExprAnd {
        operands: vec![
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(1), BinaryOp::Le, 5i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn range_to_equality_with_other_operands() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `x and a >= 5 and a <= 5` → `x and a = 5`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::arg(0),
            Expr::binary_op(Expr::arg(1), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(1), BinaryOp::Le, 5i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none()); // Still has multiple operands
    assert_eq!(expr.operands.len(), 2);

    // One should be arg(0), the other should be the equality
    let has_equality = expr
        .operands
        .iter()
        .any(|e| matches!(e, Expr::BinaryOp(op) if op.op.is_eq()));
    assert!(has_equality);
}

#[test]
fn range_to_equality_uneven_repetitions() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a >= 5 and a >= 5 and a <= 5` → `a = 5`
    let mut expr = ExprAnd {
        operands: vec![
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
        ],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    // All bounds collapse to a single equality
    let Some(Expr::BinaryOp(bin_op)) = result else {
        panic!("expected binary op");
    };
    assert!(bin_op.op.is_eq());
}

// Null propagation tests

#[test]
fn null_and_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and null` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::null()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_and_true_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and true` → `null` (true is removed, leaving only null)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), true.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn true_and_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `true and null` → `null` (true is removed, leaving only null)
    let mut expr = ExprAnd {
        operands: vec![true.into(), Expr::null()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_and_false_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and false` → `false` (false short-circuits)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), false.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn false_and_null_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `false and null` → `false` (false short-circuits)
    let mut expr = ExprAnd {
        operands: vec![false.into(), Expr::null()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn null_and_symbolic_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and a` → no change (symbolic operand present)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn multiple_nulls_become_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and null and null` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::null(), Expr::null()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn multiple_nulls_and_true_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null and true and null and true` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), true.into(), Expr::null(), true.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

// Error operand tests

#[test]
fn error_operand_not_simplified_in_and() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(error("boom"), arg(0))` → no simplification.
    // Error represents an unreachable branch; it does not poison the AND.
    // In practice, other operands (guards) will drive the AND to false.
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn error_and_true_keeps_error() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(error("boom"), true)` → `error("boom")` (true is removed, error remains)
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), true.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(matches!(&result.unwrap(), Expr::Error(e) if e.message == "boom"));
}

#[test]
fn error_and_false_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `and(error("boom"), false)` → `false` (false short-circuits AND)
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), false.into()],
    };
    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

// --- AND-over-OR branch pruning tests ---

/// The core enum variant+field pattern: disc == 1 in the outer AND
/// contradicts disc != 1 in the else branch of the OR.
///
/// AND(disc == 1, OR(AND(disc == 1, addr == "alice"), AND(disc != 1, Error == "alice")))
///   → AND(disc == 1, addr == "alice")
#[test]
fn prune_or_branch_contradicting_outer_eq() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    let disc_eq_1 = Expr::eq(Expr::arg(0), Expr::from(1i64));
    let addr_eq_alice = Expr::eq(Expr::arg(1), Expr::from("alice"));
    let disc_ne_1 = Expr::ne(Expr::arg(0), Expr::from(1i64));
    let error_eq_alice = Expr::eq(Expr::error("unreachable"), Expr::from("alice"));

    let mut expr = ExprAnd {
        operands: vec![
            disc_eq_1.clone(),
            Expr::Or(ExprOr {
                operands: vec![
                    Expr::and(disc_eq_1.clone(), addr_eq_alice.clone()),
                    Expr::and(disc_ne_1, error_eq_alice),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    // Should simplify: OR collapses to single branch, then AND flattens
    // and deduplicates disc == 1.
    assert!(result.is_none()); // Still has 2 operands
    assert_eq!(expr.operands.len(), 2);

    // The two remaining operands should be disc == 1 and addr == "alice"
    assert!(expr.operands.contains(&disc_eq_1));
    assert!(expr.operands.contains(&addr_eq_alice));
}

/// Multiple OR branches where only the matching one survives.
///
/// AND(x == 1, OR(AND(x == 1, a), AND(x == 2, b), AND(x == 3, c)))
///   → AND(x == 1, a)
#[test]
fn prune_or_multiple_contradicting_branches() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    let x_eq_1 = Expr::eq(Expr::arg(0), Expr::from(1i64));
    let x_eq_2 = Expr::eq(Expr::arg(0), Expr::from(2i64));
    let x_eq_3 = Expr::eq(Expr::arg(0), Expr::from(3i64));

    let mut expr = ExprAnd {
        operands: vec![
            x_eq_1.clone(),
            Expr::Or(ExprOr {
                operands: vec![
                    Expr::and(x_eq_1.clone(), Expr::arg(1)),
                    Expr::and(x_eq_2, Expr::arg(2)),
                    Expr::and(x_eq_3, Expr::arg(3)),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(expr.operands.contains(&x_eq_1));
    assert!(expr.operands.contains(&Expr::arg(1)));
}

/// When no OR branch contradicts the outer constraint, nothing is pruned.
#[test]
fn prune_or_no_contradiction_preserved() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // AND(x == 1, OR(AND(y == 2, a), AND(y == 3, b)))
    // x == 1 doesn't contradict y == 2 or y == 3 — no pruning.
    let mut expr = ExprAnd {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::from(1i64)),
            Expr::Or(ExprOr {
                operands: vec![
                    Expr::and(
                        Expr::eq(Expr::arg(1), Expr::from(2i64)),
                        Expr::arg(2),
                    ),
                    Expr::and(
                        Expr::eq(Expr::arg(1), Expr::from(3i64)),
                        Expr::arg(3),
                    ),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    // No simplification — 2 operands remain, OR still has 2 branches.
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(matches!(&expr.operands[1], Expr::Or(or) if or.operands.len() == 2));
}

/// All OR branches pruned → OR becomes false → AND becomes false.
#[test]
fn prune_or_all_branches_contradicted() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // AND(x == 1, OR(AND(x == 2, a), AND(x == 3, b)))
    // Both branches contradict x == 1, so OR → false → AND → false.
    let mut expr = ExprAnd {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::from(1i64)),
            Expr::Or(ExprOr {
                operands: vec![
                    Expr::and(
                        Expr::eq(Expr::arg(0), Expr::from(2i64)),
                        Expr::arg(1),
                    ),
                    Expr::and(
                        Expr::eq(Expr::arg(0), Expr::from(3i64)),
                        Expr::arg(2),
                    ),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

/// A non-AND branch in the OR is tested as a single-element constraint.
///
/// AND(x == 1, OR(x == 2, AND(x == 1, a)))
///   → prune `x == 2` (contradicts x == 1)
///   → AND(x == 1, a)
#[test]
fn prune_or_non_and_branch() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    let x_eq_1 = Expr::eq(Expr::arg(0), Expr::from(1i64));
    let x_eq_2 = Expr::eq(Expr::arg(0), Expr::from(2i64));

    let mut expr = ExprAnd {
        operands: vec![
            x_eq_1.clone(),
            Expr::Or(ExprOr {
                operands: vec![
                    x_eq_2, // bare expr, not wrapped in AND
                    Expr::and(x_eq_1.clone(), Expr::arg(1)),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(expr.operands.contains(&x_eq_1));
    assert!(expr.operands.contains(&Expr::arg(1)));
}

/// Multiple non-OR operands form the constraint set.
///
/// AND(x == 1, y == 2, OR(AND(x == 1, y == 3, a), AND(x == 1, y == 2, b)))
///   → prune first OR branch (y == 2 contradicts y == 3)
///   → AND(x == 1, y == 2, b)
#[test]
fn prune_or_multiple_constraints() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    let x_eq_1 = Expr::eq(Expr::arg(0), Expr::from(1i64));
    let y_eq_2 = Expr::eq(Expr::arg(1), Expr::from(2i64));
    let y_eq_3 = Expr::eq(Expr::arg(1), Expr::from(3i64));

    let mut expr = ExprAnd {
        operands: vec![
            x_eq_1.clone(),
            y_eq_2.clone(),
            Expr::Or(ExprOr {
                operands: vec![
                    Expr::and_from_vec(vec![x_eq_1.clone(), y_eq_3, Expr::arg(2)]),
                    Expr::and_from_vec(vec![x_eq_1.clone(), y_eq_2.clone(), Expr::arg(3)]),
                ],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    // After pruning + flatten + dedup: AND(x == 1, y == 2, arg(3))
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 3);
    assert!(expr.operands.contains(&x_eq_1));
    assert!(expr.operands.contains(&y_eq_2));
    assert!(expr.operands.contains(&Expr::arg(3)));
}

/// When the AND has no non-OR operands, no pruning occurs.
#[test]
fn prune_or_no_constraints_no_change() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // AND(OR(a, b), OR(c, d)) — no non-OR constraints to propagate.
    let mut expr = ExprAnd {
        operands: vec![
            Expr::Or(ExprOr {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::Or(ExprOr {
                operands: vec![Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };

    let result = simplify.simplify_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

/// End-to-end through visit_expr_mut: the full enum variant+field pattern.
///
/// AND(disc == 1, eq(Match(disc, [1 => addr, 2 => arg(2)], else: Error), "alice"))
///   → match elimination produces OR
///   → AND-over-OR pruning removes else branch
///   → AND(disc == 1, addr == "alice")
#[test]
fn prune_or_end_to_end_via_visit() {
    use toasty_core::stmt::{ExprMatch, MatchArm, Value, VisitMut};

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    let disc = Expr::arg(0);
    let addr = Expr::arg(1);

    let mut expr = Expr::and(
        Expr::eq(disc.clone(), Expr::from(1i64)),
        Expr::eq(
            Expr::Match(ExprMatch {
                subject: Box::new(disc.clone()),
                arms: vec![
                    MatchArm {
                        pattern: Value::from(1i64),
                        expr: addr.clone(),
                    },
                    MatchArm {
                        pattern: Value::from(2i64),
                        expr: Expr::arg(2),
                    },
                ],
                else_expr: Box::new(Expr::error("unreachable")),
            }),
            Expr::from("alice"),
        ),
    );

    simplify.visit_expr_mut(&mut expr);

    // Result should be AND(disc == 1, addr == "alice")
    let Expr::And(and) = &expr else {
        panic!("expected AND, got: {expr:?}");
    };
    assert_eq!(and.operands.len(), 2);
    assert!(and.operands.contains(&Expr::eq(disc, Expr::from(1i64))));
    assert!(and.operands.contains(&Expr::eq(addr, Expr::from("alice"))));
}
