use toasty_core::stmt::{BinaryOp, Expr, Value};

#[test]
fn evaluates_read_driver_predicates() {
    assert_eq!(
        Expr::between(10_i64, 5_i64, 10_i64).eval_const().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::starts_with("markdown", "mark").eval_const().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::like("readme.md", "%.md").eval_const().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::ilike("README.MD", "readme._d").eval_const().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::ilike("Straße", "STRASSE").eval_const().unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::any_op(3_i64, BinaryOp::Gt, Expr::list([1_i64, 4_i64]),)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        Expr::all_op(3_i64, BinaryOp::Gt, Expr::list([1_i64, 2_i64]),)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn ordered_predicates_treat_null_as_not_matching() {
    assert_eq!(
        Expr::between(Value::Null, 1_i64, 2_i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn escaped_like_patterns_match_literals() {
    assert_eq!(
        Expr::like_with_escape("100%", "100\\%", '\\')
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
    assert!(
        Expr::like_with_escape("value", "value\\", '\\')
            .eval_const()
            .is_err()
    );
}
