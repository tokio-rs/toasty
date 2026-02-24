use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{BinaryOp, Expr, ExprAnd};

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
    use toasty_core::stmt::ExprOr;

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
    use toasty_core::stmt::ExprOr;

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
    use toasty_core::stmt::ExprOr;

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
