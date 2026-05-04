use crate::engine::fold::expr_record::fold_expr_record;
use toasty_core::stmt::{Expr, ExprRecord, Value};

#[test]
fn all_const_values_become_value_record() {
    // `record([1, "hello", true]) → {1, "hello", true}`
    let mut expr = ExprRecord {
        fields: vec![
            Expr::Value(Value::from(1i64)),
            Expr::Value(Value::from("hello")),
            Expr::Value(Value::from(true)),
        ],
    };
    let result = fold_expr_record(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::Record(record)) = result.unwrap() else {
        panic!("expected result to be a `Value::Record`");
    };
    assert_eq!(record.len(), 3);
    assert_eq!(record[0], Value::from(1i64));
    assert_eq!(record[1], Value::from("hello"));
    assert_eq!(record[2], Value::from(true));
}

#[test]
fn empty_record_simplified() {
    // `record([]) → {}`
    let mut expr = ExprRecord { fields: vec![] };
    let result = fold_expr_record(&mut expr);

    assert!(result.is_some());
    let Expr::Value(Value::Record(record)) = result.unwrap() else {
        panic!("expected result to be a `Value::Record`");
    };
    assert!(record.is_empty());
}

#[test]
fn non_const_not_simplified() {
    // `record([1, arg(0)])`, non-constant, not simplified
    let mut expr = ExprRecord {
        fields: vec![Expr::Value(Value::from(1i64)), Expr::arg(0)],
    };
    let result = fold_expr_record(&mut expr);

    assert!(result.is_none());
}

#[test]
fn record_with_error_field_not_simplified() {
    // `record([1, error("boom")])` — error is not a Value, so not folded
    let mut expr = ExprRecord {
        fields: vec![Expr::Value(Value::from(1i64)), Expr::error("boom")],
    };
    let result = fold_expr_record(&mut expr);

    assert!(result.is_none());
}
