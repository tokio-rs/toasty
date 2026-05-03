use crate::engine::fold::expr_or::fold_expr_or;
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
    // `or(A, or(B, C)) → or(A, B, C)`
    let mut expr = nested_or(Expr::arg(0), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_none()); // Modified in place
    assert_eq!(expr.operands.len(), 3);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(1));
    assert_eq!(expr.operands[2], Expr::arg(2));
}

#[test]
fn flatten_with_false_in_outer() {
    // `or(false, or(B, C)) → or(B, C)`
    let mut expr = nested_or(false.into(), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(1));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_with_false_in_nested_first() {
    // `or(A, or(false, C)) → or(A, C)`
    let mut expr = nested_or(Expr::arg(0), false.into(), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert_eq!(expr.operands[0], Expr::arg(0));
    assert_eq!(expr.operands[1], Expr::arg(2));
}

#[test]
fn flatten_outer_false_nested_one_false() {
    // `or(false, or(false, C)) → C`
    let mut expr = nested_or(false.into(), false.into(), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(2));
}

#[test]
fn flatten_outer_symbolic_nested_all_false() {
    // `or(A, or(false, false)) → A`
    let mut expr = nested_or(Expr::arg(0), false.into(), false.into());
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn flatten_all_false() {
    // `or(false, or(false, false)) → false`
    let mut expr = nested_or(false.into(), false.into(), false.into());
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn flatten_with_true_in_outer() {
    // `or(true, or(B, C)) → true`
    let mut expr = nested_or(true.into(), Expr::arg(1), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn flatten_with_true_in_nested() {
    // `or(A, or(true, C)) → true`
    let mut expr = nested_or(Expr::arg(0), true.into(), Expr::arg(2));
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn single_operand_unwrapped() {
    // `or(arg(0)) → arg(0)`
    let mut expr = ExprOr {
        operands: vec![Expr::arg(0)],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert_eq!(result.unwrap(), Expr::arg(0));
}

#[test]
fn empty_after_removing_false() {
    // `or(false, false) → false`
    let mut expr = ExprOr {
        operands: vec![false.into(), false.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

// Null propagation tests

#[test]
fn null_or_null_becomes_null() {
    // `null or null` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::null()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_or_false_becomes_null() {
    // `null or false` → `null` (false is removed, leaving only null)
    let mut expr = ExprOr {
        operands: vec![Expr::null(), false.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn false_or_null_becomes_null() {
    // `false or null` → `null` (false is removed, leaving only null)
    let mut expr = ExprOr {
        operands: vec![false.into(), Expr::null()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_or_true_becomes_true() {
    // `null or true` → `true`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), true.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn null_or_symbolic_not_simplified() {
    // `null or a` → no change (symbolic operand present)
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::arg(0)],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
}

#[test]
fn multiple_nulls_become_null() {
    // `null or null or null` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), Expr::null(), Expr::null()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn multiple_nulls_and_false_becomes_null() {
    // `null or false or null or false` → `null`
    let mut expr = ExprOr {
        operands: vec![Expr::null(), false.into(), Expr::null(), false.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

// Error operand tests

#[test]
fn error_operand_preserved_in_or() {
    // `or(error("boom"), arg(0))` → no simplification (error is not true/false/null)
    let mut expr = ExprOr {
        operands: vec![Expr::error("boom"), Expr::arg(0)],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_none());
    assert_eq!(expr.operands.len(), 2);
    assert!(matches!(&expr.operands[0], Expr::Error(_)));
}

#[test]
fn error_or_false_keeps_error() {
    // `or(error("boom"), false)` → `error("boom")` (false is removed, error remains)
    let mut expr = ExprOr {
        operands: vec![Expr::error("boom"), false.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(matches!(&result.unwrap(), Expr::Error(e) if e.message == "boom"));
}

#[test]
fn error_or_true_becomes_true() {
    // `or(error("boom"), true)` → `true` (true short-circuits OR)
    let mut expr = ExprOr {
        operands: vec![Expr::error("boom"), true.into()],
    };
    let result = fold_expr_or(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}
