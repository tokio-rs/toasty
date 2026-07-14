use crate::engine::fold::expr_and::fold_expr_and;
use toasty_core::stmt::{Expr, ExprAnd};

/// Builds `and(a, and(b, c))`, a nested AND structure for testing flattening.
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
    // `and(A, and(B, C)) → and(A, B, C)`
    let mut expr = nested_and(Expr::arg(0), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none()); // Modified in place
    assert_eq!(expr.operands.len(), 3);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
    assert_eq!(expr.operands[2], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_outer() {
    // `and(true, and(B, C)) → and(B, C)`
    let mut expr = nested_and(true.into(), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(1));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_nested_first() {
    // `and(A, and(true, C)) → and(A, C)`
    let mut expr = nested_and(Expr::arg(0), true.into(), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_true_in_nested_second() {
    // `and(A, and(B, true)) → and(A, B)`
    let mut expr = nested_and(Expr::arg(0), Expr::arg(1), true.into());
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
}

#[test]
fn flatten_outer_true_nested_one_true() {
    // `and(true, and(true, C)) → C`
    let mut expr = nested_and(true.into(), true.into(), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(2));
}

#[test]
fn flatten_outer_symbolic_nested_all_true() {
    // `and(A, and(true, true)) → A`
    let mut expr = nested_and(Expr::arg(0), true.into(), true.into());
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn flatten_all_true() {
    // `and(true, and(true, true)) → true`
    let mut expr = nested_and(true.into(), true.into(), true.into());
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn flatten_with_false_in_outer() {
    // `and(false, and(B, C)) → false`
    let mut expr = nested_and(false.into(), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_with_false_in_nested() {
    // `and(A, and(false, C)) → false`
    let mut expr = nested_and(Expr::arg(0), false.into(), Expr::arg(2));
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_true_and_false_mixed() {
    // `and(true, and(false, true)) → false`
    let mut expr = nested_and(true.into(), false.into(), true.into());
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn single_operand_unwrapped() {
    // `and(arg(0)) → arg(0)`
    let mut expr = ExprAnd {
        operands: vec![Expr::arg(0)],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn empty_after_removing_true() {
    // `and(true, true) → true`
    let mut expr = ExprAnd {
        operands: vec![true.into(), true.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

// Null propagation tests

#[test]
fn null_and_null_becomes_null() {
    // `null and null` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::null()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_and_true_becomes_null() {
    // `null and true` → `null` (true is removed, leaving only null)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), true.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn true_and_null_becomes_null() {
    // `true and null` → `null` (true is removed, leaving only null)
    let mut expr = ExprAnd {
        operands: vec![true.into(), Expr::null()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_and_false_becomes_false() {
    // `null and false` → `false` (false short-circuits)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), false.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn false_and_null_becomes_false() {
    // `false and null` → `false` (false short-circuits)
    let mut expr = ExprAnd {
        operands: vec![false.into(), Expr::null()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn null_and_symbolic_not_simplified() {
    // `null and a` → no change (symbolic operand present)
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::arg(0)],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn multiple_nulls_become_null() {
    // `null and null and null` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), Expr::null(), Expr::null()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn multiple_nulls_and_true_becomes_null() {
    // `null and true and null and true` → `null`
    let mut expr = ExprAnd {
        operands: vec![Expr::null(), true.into(), Expr::null(), true.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

// Error operand tests

#[test]
fn error_operand_not_simplified_in_and() {
    // `and(error("boom"), arg(0))` → no simplification.
    // Error represents an unreachable branch; it does not poison the AND.
    // In practice, other operands (guards) will drive the AND to false.
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), Expr::arg(0)],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn error_and_true_keeps_error() {
    // `and(error("boom"), true)` → `error("boom")` (true is removed, error remains)
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), true.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(matches!(&result.unwrap(), Expr::Error(e) if e.message == "boom"));
}

#[test]
fn error_and_false_becomes_false() {
    // `and(error("boom"), false)` → `false` (false short-circuits AND)
    let mut expr = ExprAnd {
        operands: vec![Expr::error("boom"), false.into()],
    };
    let result = fold_expr_and(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}
