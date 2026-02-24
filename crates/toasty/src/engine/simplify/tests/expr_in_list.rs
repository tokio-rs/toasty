use super::test_schema;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{self, Expr, Value};

/// Helper to construct an "in list" expression.
fn in_list(lhs: Expr, list: Expr) -> stmt::ExprInList {
    stmt::ExprInList {
        expr: Box::new(lhs),
        list: Box::new(list),
    }
}

/// Helper for making a value list.
fn value_list(values: Vec<Value>) -> Expr {
    Expr::Value(Value::List(values))
}

/// Helper for making an expression list.
fn expr_list(items: Vec<Expr>) -> Expr {
    Expr::List(stmt::ExprList { items })
}

#[test]
fn empty_value_list_becomes_false() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), []) → false`
    let mut expr = in_list(Expr::arg(0), value_list(vec![]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn empty_expr_list_becomes_false() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), list([])) → false`
    let mut expr = in_list(Expr::arg(0), expr_list(vec![]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn single_value_becomes_eq() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), [42]) → eq(arg(0), 42)`
    let mut expr = in_list(Expr::arg(0), value_list(vec![Value::from(42i64)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(matches!(
        result,
        Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
            if *lhs == Expr::arg(0) && *rhs == Expr::Value(Value::from(42i64))
    ));
}

#[test]
fn single_expr_becomes_eq() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), list([arg(1)])) → eq(arg(0), arg(1))`
    let mut expr = in_list(Expr::arg(0), expr_list(vec![Expr::arg(1)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(matches!(
        result,
        Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
            if *lhs == Expr::arg(0) && *rhs == Expr::arg(1)
    ));
}

#[test]
fn two_values_unchanged() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), [1, 2])`, multiple items, not simplified
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![Value::from(1i64), Value::from(2i64)]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
}

#[test]
fn two_exprs_unchanged() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), list([arg(1), arg(2)]))`, multiple items, not simplified
    let mut expr = in_list(Expr::arg(0), expr_list(vec![Expr::arg(1), Expr::arg(2)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
}

#[test]
fn arg_in_single() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), [42]) → eq(arg(0), 42)`
    let mut expr = in_list(Expr::arg(0), value_list(vec![Value::from(42i64)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(matches!(
        result,
        Some(Expr::BinaryOp(stmt::ExprBinaryOp { lhs, op: stmt::BinaryOp::Eq, rhs }))
            if *lhs == Expr::arg(0) && *rhs == Expr::Value(Value::from(42i64))
    ));
}

#[test]
fn arg_in_empty() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), []) → false`
    let mut expr = in_list(Expr::arg(0), value_list(vec![]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_false());
}

#[test]
fn arg_in_multi() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `in_list(arg(0), [1, 2])`, multiple items, not simplified
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![Value::from(1i64), Value::from(2i64)]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
}

// Null propagation tests

#[test]
fn null_in_list_becomes_null() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `null in (1, 2, 3)` → `null`
    let mut expr = in_list(
        Expr::null(),
        value_list(vec![
            Value::from(1i64),
            Value::from(2i64),
            Value::from(3i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_in_single_item_becomes_null() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `null in (42)` → `null`
    let mut expr = in_list(Expr::null(), value_list(vec![Value::from(42i64)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

#[test]
fn null_in_expr_list_becomes_null() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `null in list([arg(0), arg(1)])` → `null`
    let mut expr = in_list(Expr::null(), expr_list(vec![Expr::arg(0), Expr::arg(1)]));
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_some());
    assert!(result.unwrap().is_value_null());
}

// Deduplication tests

#[test]
fn dedup_all_identical_values() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `x in (1, 1, 1)` → `x = 1`
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![
            Value::from(1i64),
            Value::from(1i64),
            Value::from(1i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    // Dedup leaves one item, which is then rewritten to equality
    assert!(matches!(result, Some(Expr::BinaryOp(_))));
}

#[test]
fn dedup_leading_duplicate() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `x in (1, 1, 2)` → `x in (1, 2)`
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![
            Value::from(1i64),
            Value::from(1i64),
            Value::from(2i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
    let Expr::Value(Value::List(values)) = &*expr.list else {
        panic!("expected Value::List");
    };
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Value::from(1i64));
    assert_eq!(values[1], Value::from(2i64));
}

#[test]
fn dedup_trailing_duplicate() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `x in (1, 2, 2)` → `x in (1, 2)`
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![
            Value::from(1i64),
            Value::from(2i64),
            Value::from(2i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
    let Expr::Value(Value::List(values)) = &*expr.list else {
        panic!("expected Value::List");
    };
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Value::from(1i64));
    assert_eq!(values[1], Value::from(2i64));
}

#[test]
fn dedup_preserves_order() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `x in (3, 1, 2, 1, 3)` → `x in (3, 1, 2)` — first occurrence wins
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![
            Value::from(3i64),
            Value::from(1i64),
            Value::from(2i64),
            Value::from(1i64),
            Value::from(3i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
    let Expr::Value(Value::List(values)) = &*expr.list else {
        panic!("expected Value::List");
    };
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Value::from(3i64));
    assert_eq!(values[1], Value::from(1i64));
    assert_eq!(values[2], Value::from(2i64));
}

#[test]
fn dedup_no_duplicates_unchanged() {
    let schema = test_schema();
    let simplify = Simplify::new(&schema);

    // `x in (1, 2, 3)` — no duplicates, list unchanged
    let mut expr = in_list(
        Expr::arg(0),
        value_list(vec![
            Value::from(1i64),
            Value::from(2i64),
            Value::from(3i64),
        ]),
    );
    let result = simplify.simplify_expr_in_list(&mut expr);

    assert!(result.is_none());
    let Expr::Value(Value::List(values)) = &*expr.list else {
        panic!("expected Value::List");
    };
    assert_eq!(values.len(), 3);
}
