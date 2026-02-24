use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{BinaryOp, Expr, ExprBinaryOp, ExprNot, Value};

fn not_expr(expr: Expr) -> ExprNot {
    ExprNot {
        expr: Box::new(expr),
    }
}

// Double negation elimination tests

#[test]
fn double_negation_eliminated() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(not(true))` → `true`
    let inner = Expr::not(Expr::Value(Value::Bool(true)));
    let mut outer = not_expr(inner);

    let result = simplify.simplify_expr_not(&mut outer);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn triple_negation_reduces_to_single() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(not(not(true)))` → `not(true)`
    let inner = Expr::not(Expr::not(Expr::Value(Value::Bool(true))));
    let mut outer = not_expr(inner);

    let result = simplify.simplify_expr_not(&mut outer);
    assert!(matches!(result, Some(Expr::Not(_))));
}

// Constant folding tests

#[test]
fn not_true_becomes_false() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(true)` → `false`
    let mut expr = not_expr(Expr::Value(Value::Bool(true)));

    let result = simplify.simplify_expr_not(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
}

#[test]
fn not_false_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(false)` → `true`
    let mut expr = not_expr(Expr::Value(Value::Bool(false)));

    let result = simplify.simplify_expr_not(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Bool(true)))));
}

#[test]
fn not_null_becomes_null() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(null)` → `null`
    let mut expr = not_expr(Expr::null());

    let result = simplify.simplify_expr_not(&mut expr);

    assert!(matches!(result, Some(Expr::Value(Value::Null))));
}

// Negation of comparison tests

#[test]
fn not_eq_becomes_ne() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 = 2)` → `1 != 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Eq,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Ne);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

#[test]
fn not_ne_becomes_eq() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 != 2)` → `1 = 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Ne,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Eq);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

#[test]
fn not_lt_becomes_ge() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 < 2)` → `1 >= 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Lt,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Ge);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

#[test]
fn not_ge_becomes_lt() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 >= 2)` → `1 < 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Ge,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Lt);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

#[test]
fn not_gt_becomes_le() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 > 2)` → `1 <= 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Gt,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Le);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

#[test]
fn not_le_becomes_gt() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(1 <= 2)` → `1 > 2`
    let mut expr = not_expr(Expr::BinaryOp(ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from(1i64))),
        op: BinaryOp::Le,
        rhs: Box::new(Expr::Value(Value::from(2i64))),
    }));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::BinaryOp(binary_op)) = result else {
        panic!("expected `BinaryOp`");
    };
    assert_eq!(binary_op.op, BinaryOp::Gt);
    assert_eq!(*binary_op.lhs, Expr::Value(Value::from(1i64)));
    assert_eq!(*binary_op.rhs, Expr::Value(Value::from(2i64)));
}

// De Morgan's law tests

#[test]
fn not_and_becomes_or_of_nots() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(a and b)` → `not(a) or not(b)`
    let mut expr = not_expr(Expr::and(Expr::arg(0), Expr::arg(1)));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::Or(or_expr)) = result else {
        panic!("expected `Or`");
    };
    assert_eq!(or_expr.operands.len(), 2);
    assert!(matches!(&or_expr.operands[0], Expr::Not(_)));
    assert!(matches!(&or_expr.operands[1], Expr::Not(_)));
}

#[test]
fn not_or_becomes_and_of_nots() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(a or b)` → `not(a) and not(b)`
    let mut expr = not_expr(Expr::or(Expr::arg(0), Expr::arg(1)));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::And(and_expr)) = result else {
        panic!("expected `And`");
    };
    assert_eq!(and_expr.operands.len(), 2);
    assert!(matches!(&and_expr.operands[0], Expr::Not(_)));
    assert!(matches!(&and_expr.operands[1], Expr::Not(_)));
}

#[test]
fn not_and_with_three_operands() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(a and b and c)` → `not(a) or not(b) or not(c)`
    let mut expr = not_expr(Expr::and_from_vec(vec![
        Expr::arg(0),
        Expr::arg(1),
        Expr::arg(2),
    ]));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::Or(or_expr)) = result else {
        panic!("expected `Or`");
    };
    assert_eq!(or_expr.operands.len(), 3);
    for operand in &or_expr.operands {
        assert!(matches!(operand, Expr::Not(_)));
    }
}

#[test]
fn not_or_with_three_operands() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(a or b or c)` → `not(a) and not(b) and not(c)`
    let mut expr = not_expr(Expr::or_from_vec(vec![
        Expr::arg(0),
        Expr::arg(1),
        Expr::arg(2),
    ]));

    let result = simplify.simplify_expr_not(&mut expr);

    let Some(Expr::And(and_expr)) = result else {
        panic!("expected `And`");
    };
    assert_eq!(and_expr.operands.len(), 3);
    for operand in &and_expr.operands {
        assert!(matches!(operand, Expr::Not(_)));
    }
}

// NOT IN tests

#[test]
fn not_in_empty_list_becomes_true() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(x in ())` → `true`
    let in_list_expr = Expr::in_list(Expr::arg(0), Expr::list::<Expr>(vec![]));
    let mut expr = not_expr(in_list_expr);

    let result = simplify.simplify_expr_not(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_true());
}

#[test]
fn not_in_non_empty_list_not_simplified() {
    let schema = test_schema();
    let mut simplify = Simplify::new(&schema);

    // `not(x in (1, 2))` → not simplified directly by expr_not
    let in_list_expr = Expr::in_list(
        Expr::arg(0),
        Expr::list(vec![Value::from(1i64), Value::from(2i64)]),
    );
    let mut expr = not_expr(in_list_expr);

    let result = simplify.simplify_expr_not(&mut expr);

    assert!(result.is_none());
}
