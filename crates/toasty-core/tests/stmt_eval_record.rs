use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Empty record
// ---------------------------------------------------------------------------

#[test]
fn eval_record_empty() {
    assert_eq!(
        Expr::record(std::iter::empty::<Expr>())
            .eval_const()
            .unwrap(),
        Value::record_from_vec(vec![])
    );
}

// ---------------------------------------------------------------------------
// Single-field record
// ---------------------------------------------------------------------------

#[test]
fn eval_record_one_i64() {
    assert_eq!(
        Expr::record([1i64]).eval_const().unwrap(),
        Value::record_from_vec(vec![Value::I64(1)])
    );
}

#[test]
fn eval_record_one_string() {
    assert_eq!(
        Expr::record(["hello"]).eval_const().unwrap(),
        Value::record_from_vec(vec![Value::from("hello")])
    );
}

// ---------------------------------------------------------------------------
// Multi-field records
// ---------------------------------------------------------------------------

#[test]
fn eval_record_two_fields() {
    assert_eq!(
        Expr::record([Expr::from(true), Expr::from(42i64)])
            .eval_const()
            .unwrap(),
        Value::record_from_vec(vec![Value::Bool(true), Value::I64(42)])
    );
}

#[test]
fn eval_record_three_fields() {
    assert_eq!(
        Expr::record([Expr::from(1i64), Expr::from("hi"), Expr::from(Value::Null)])
            .eval_const()
            .unwrap(),
        Value::record_from_vec(vec![Value::I64(1), Value::from("hi"), Value::Null])
    );
}

// ---------------------------------------------------------------------------
// Fields are expressions — they are evaluated, not stored raw
// ---------------------------------------------------------------------------

#[test]
fn eval_record_fields_are_evaluated() {
    // A field that is itself an expression (is_null)
    let expr = Expr::record([Expr::is_null(Value::Null), Expr::from(1i64)]);
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::record_from_vec(vec![Value::Bool(true), Value::I64(1)])
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::record([Expr::from(1i64), Expr::from("a")]);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
