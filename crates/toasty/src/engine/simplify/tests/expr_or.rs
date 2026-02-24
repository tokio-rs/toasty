use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprOr};

/// Builds `or(a, or(b, c))`, a nested OR structure for testing flattening.
fn nested_or(a: Expr, b: Expr, c: Expr) -> ExprOr {
    ExprOr {
        operands: vec![
            a,
            Expr::Or(ExprOr {
                operands: vec![b, c],
            }),
        ],
    }
}

#[test]
fn flatten_all_symbolic() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(A, or(B, C)) → or(A, B, C)`
    let mut expr = nested_or(Expr::arg(0), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 3);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
    assert_eq!(expr.operands[2], Expr::arg(2));
}

#[test]
fn flatten_with_false_in_outer() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(false, or(B, C)) → or(B, C)`
    let mut expr = nested_or(false.into(), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(1));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_false_in_nested_first() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(A, or(false, C)) → or(A, C)`
    let mut expr = nested_or(Expr::arg(0), false.into(), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_outer_false_nested_one_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(false, or(false, C)) → C`
    let mut expr = nested_or(false.into(), false.into(), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(2));
}

#[test]
fn flatten_outer_symbolic_nested_all_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(A, or(false, false)) → A`
    let mut expr = nested_or(Expr::arg(0), false.into(), false.into());
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn flatten_all_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(false, or(false, false)) → false`
    let mut expr = nested_or(false.into(), false.into(), false.into());
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_with_true_in_outer() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(true, or(B, C)) → true`
    let mut expr = nested_or(true.into(), Expr::arg(1), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn flatten_with_true_in_nested() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(A, or(true, C)) → true`
    let mut expr = nested_or(Expr::arg(0), true.into(), Expr::arg(2));
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn single_operand_unwrapped() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(arg(0)) → arg(0)`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_two_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, a) → a`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_three_identical() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, a, a) → a`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(0), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn idempotent_with_different() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, b, a) → or(a, b)`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_or_and() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, and(a, b))` → `a`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn absorption_with_multiple_operands() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, b, and(a, c))` → `or(a, b)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn absorption_two_or_three_and() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `or(a, b, and(a, c, d))` → `or(a, b)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::arg(0),
            Expr::arg(1),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn factoring_basic() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `(a and b) or (a and c)` → `a and (b or c)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and (b or c)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 2);
    assert_eq!(and_expr.operands[0], Expr::arg(0));

    let Expr::Or(or_expr) = &and_expr.operands[1] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 2);
    assert_eq!(or_expr.operands[0], Expr::arg(1));
    assert_eq!(or_expr.operands[1], Expr::arg(2));
}

#[test]
fn factoring_multiple_common() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `(a and b and c) or (a and b and d)` → `a and b and (c or d)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(2)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and b and (c or d)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 3);
    assert_eq!(and_expr.operands[0], Expr::arg(0));
    assert_eq!(and_expr.operands[1], Expr::arg(1));

    let Expr::Or(or_expr) = &and_expr.operands[2] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 2);
    assert_eq!(or_expr.operands[0], Expr::arg(2));
    assert_eq!(or_expr.operands[1], Expr::arg(3));
}

#[test]
fn factoring_three_ands() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `(a and b) or (a and c) or (a and d)` → `a and (b or c or d)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(2)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Result should be `a and (b or c or d)`
    let Some(Expr::And(and_expr)) = result else {
        panic!("expected And");
    };
    assert_eq!(and_expr.operands.len(), 2);
    assert_eq!(and_expr.operands[0], Expr::arg(0));

    let Expr::Or(or_expr) = &and_expr.operands[1] else {
        panic!("expected Or");
    };
    assert_eq!(or_expr.operands.len(), 3);
    assert_eq!(or_expr.operands[0], Expr::arg(1));
    assert_eq!(or_expr.operands[1], Expr::arg(2));
    assert_eq!(or_expr.operands[2], Expr::arg(3));
}

#[test]
fn factoring_no_common() {
    use toasty_core::stmt::ExprAnd;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `(a and b) or (c and d)` → no change (no common factor)
    let mut expr = ExprOr {
        operands: vec![
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(0), Expr::arg(1)],
            }),
            Expr::And(ExprAnd {
                operands: vec![Expr::arg(2), Expr::arg(3)],
            }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
}

#[test]
fn complement_basic() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a or not(a)` → `true` (where a is a non-nullable comparison)
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn complement_with_other_operands() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a or b or not(a)` → `true`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![
            a.clone(),
            Expr::arg(2),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn complement_nullable_not_simplified() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a or not(a)` where `a` is an arg (nullable) → no change
    let a = Expr::arg(0);
    let mut expr = ExprOr {
        operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
}

#[test]
fn complement_multiple_repetitions() {
    use toasty_core::stmt::ExprNot;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a or a or not(a) or not(a)` → `true`
    let a = Expr::eq(Expr::arg(0), Expr::arg(1));
    let mut expr = ExprOr {
        operands: vec![
            a.clone(),
            a.clone(),
            Expr::Not(ExprNot {
                expr: Box::new(a.clone()),
            }),
            Expr::Not(ExprNot { expr: Box::new(a) }),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

// Null propagation tests

#[test]
fn null_or_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or null` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::null()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_or_false_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or false` → `null` (false is removed, leaving only null)
    let mut expr = ExprOr {
        operands: vec![Expr::null(), false.into()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn false_or_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `false or null` → `null` (false is removed, leaving only null)
    let mut expr = ExprOr {
        operands: vec![false.into(), Expr::null()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_or_true_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or true` → `true`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), true.into()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn null_or_symbolic_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or a` → no change (symbolic operand present)
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::arg(0)],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn multiple_nulls_become_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or null or null` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::null(), Expr::null()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn multiple_nulls_and_false_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `null or false or null or false` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), false.into(), Expr::null(), false.into()],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

// OR-to-IN conversion tests

#[test]
fn or_to_in_basic() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or a = 2 or a = 3` → `a in (1, 2, 3)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(3i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    let Some(Expr::InList(in_list)) = result else {
        panic!("expected InList");
    };
    assert_eq!(*in_list.expr, Expr::arg(0));
}

#[test]
fn or_to_in_two_values() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or a = 2` → `a in (1, 2)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(matches!(result, Some(Expr::InList(_))));
}

#[test]
fn or_to_in_single_value_not_converted() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1` (single equality, not converted)
    let mut expr = ExprOr {
        operands: vec![Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64)))],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Single operand gets unwrapped, but not to IN
    assert!(result.is_some());
    assert!(matches!(result, Some(Expr::BinaryOp(_))));
}

#[test]
fn or_to_in_different_lhs_not_converted() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or b = 2` (different LHS, not converted to single IN)
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(2i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Not converted, stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn or_to_in_mixed_keeps_other_operands() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or a = 2 or b = 3` → `a in (1, 2) or b = 3`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR but with transformed operands
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // Should have one InList and one BinaryOp
    let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
    let has_binary_op = expr.operands.iter().any(|e| matches!(e, Expr::BinaryOp(_)));
    assert!(has_in_list);
    assert!(has_binary_op);
}

#[test]
fn or_to_in_multiple_groups() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or a = 2 or b = 3 or b = 4` → `a in (1, 2) or b in (3, 4)`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
            Expr::eq(Expr::arg(1), Expr::Value(Value::from(4i64))),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR with two InList operands
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // Both should be InList
    assert!(expr.operands.iter().all(|e| matches!(e, Expr::InList(_))));
}

#[test]
fn or_to_in_with_non_equality_preserved() {
    use toasty_core::stmt::Value;

    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = 1 or a = 2 or c` → `a in (1, 2) or c`
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
            Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            Expr::arg(2), // non-equality operand
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);

    // One InList, one arg
    let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
    let has_arg = expr.operands.iter().any(|e| matches!(e, Expr::Arg(_)));
    assert!(has_in_list);
    assert!(has_arg);
}

#[test]
fn or_to_in_non_const_rhs_not_converted() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `a = b or a = c` (non-constant RHS, not converted)
    let mut expr = ExprOr {
        operands: vec![
            Expr::eq(Expr::arg(0), Expr::arg(1)),
            Expr::eq(Expr::arg(0), Expr::arg(2)),
        ],
    };
    let result = simplify.simplify_expr_or(&mut expr);

    // Not converted, stays as OR
    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(expr.operands.iter().all(|e| matches!(e, Expr::BinaryOp(_))));
}
