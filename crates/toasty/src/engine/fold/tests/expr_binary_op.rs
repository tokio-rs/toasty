use crate::engine::fold::expr_binary_op::fold_expr_binary_op;
use toasty_core::stmt::{BinaryOp, Expr, Value};

#[test]
fn constant_eq_same_values_becomes_true() {
    // `eq(5, 5)` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_eq_different_values_becomes_false() {
    // `eq(1, 2)` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(2i64));

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ne_same_values_becomes_false() {
    // `ne(5, 5)` → `false`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ne_different_values_becomes_true() {
    // `ne("abc", "def")` → `true`
    let mut lhs = Expr::Value(Value::from("abc"));
    let mut rhs = Expr::Value(Value::from("def"));

    let result = fold_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_eq_with_null_becomes_null() {
    // `eq(null, 5)` → `null`
    let mut lhs = Expr::Value(Value::Null);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

#[test]
fn constant_eq_null_with_null_becomes_null() {
    // `eq(null, null)` → `null`
    let mut lhs = Expr::Value(Value::Null);
    let mut rhs = Expr::Value(Value::Null);

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

#[test]
fn constant_lt_becomes_true() {
    // `1 < 5` → `true`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_lt_becomes_false() {
    // `5 < 1` → `false`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(1i64));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_le_equal_becomes_true() {
    // `5 <= 5` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_le_becomes_false() {
    // `10 <= 5` → `false`
    let mut lhs = Expr::Value(Value::from(10i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_gt_becomes_true() {
    // `10 > 5` → `true`
    let mut lhs = Expr::Value(Value::from(10i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_gt_becomes_false() {
    // `1 > 5` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_ge_equal_becomes_true() {
    // `5 >= 5` → `true`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_ge_becomes_false() {
    // `1 >= 5` → `false`
    let mut lhs = Expr::Value(Value::from(1i64));
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn constant_lt_string_lexicographic() {
    // `"abc" < "def"` → `true` (lexicographic)
    let mut lhs = Expr::Value(Value::from("abc"));
    let mut rhs = Expr::Value(Value::from("def"));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn constant_lt_different_types_not_simplified() {
    // `5 < "abc"` is not simplified (incompatible types)
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::Value(Value::from("abc"));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn lt_with_non_constant_not_simplified() {
    // `arg(0) < 5` is not simplified (non-constant lhs, value already on right)
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}

#[test]
fn x_eq_true_becomes_x() {
    // `x = true` → `x`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(true));

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn true_eq_x_becomes_x() {
    // `true = x` → `x`
    let mut lhs = Expr::Value(Value::Bool(true));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn x_eq_false_becomes_not_x() {
    // `x = false` → `not(x)`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(false));

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Not(_))));
}

#[test]
fn x_ne_true_becomes_not_x() {
    // `x != true` → `not(x)`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(true));

    let result = fold_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Not(_))));
}

#[test]
fn x_ne_false_becomes_x() {
    // `x != false` → `x`
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::Bool(false));

    let result = fold_expr_binary_op(BinaryOp::Ne, &mut lhs, &mut rhs);

    assert!(matches!(result, Some(Expr::Arg(_))));
}

#[test]
fn canonicalize_eq_literal_on_left() {
    // `5 = x` → `x = 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Eq);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_lt_literal_on_left() {
    // `5 < x` → `x > 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Gt);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_gt_literal_on_left() {
    // `5 > x` → `x < 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Gt, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Lt);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_le_literal_on_left() {
    // `5 <= x` → `x >= 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Le, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Ge);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn canonicalize_ge_literal_on_left() {
    // `5 >= x` → `x <= 5`
    let mut lhs = Expr::Value(Value::from(5i64));
    let mut rhs = Expr::arg(0);

    let result = fold_expr_binary_op(BinaryOp::Ge, &mut lhs, &mut rhs);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected BinaryOp");
    };
    assert_eq!(binary_op.op, BinaryOp::Le);
    assert!(matches!(*binary_op.lhs, Expr::Arg(_)));
    assert!(matches!(*binary_op.rhs, Expr::Value(Value::I64(5))));
}

#[test]
fn no_canonicalize_when_literal_on_right() {
    // `x < 5` is already canonical, no change
    let mut lhs = Expr::arg(0);
    let mut rhs = Expr::Value(Value::from(5i64));

    let result = fold_expr_binary_op(BinaryOp::Lt, &mut lhs, &mut rhs);

    assert!(result.is_none());
}
