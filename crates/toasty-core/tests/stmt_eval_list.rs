use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Empty list
// ---------------------------------------------------------------------------

#[test]
fn eval_list_empty() {
    assert_eq!(
        Expr::list(std::iter::empty::<Expr>()).eval_const().unwrap(),
        Value::List(vec![])
    );
}

// ---------------------------------------------------------------------------
// Homogeneous lists
// ---------------------------------------------------------------------------

#[test]
fn eval_list_i64_values() {
    assert_eq!(
        Expr::list([1i64, 2i64, 3i64]).eval_const().unwrap(),
        Value::List(vec![Value::I64(1), Value::I64(2), Value::I64(3)])
    );
}

#[test]
fn eval_list_bool_values() {
    assert_eq!(
        Expr::list([true, false, true]).eval_const().unwrap(),
        Value::List(vec![
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true)
        ])
    );
}

#[test]
fn eval_list_string_values() {
    assert_eq!(
        Expr::list(["a", "b", "c"]).eval_const().unwrap(),
        Value::List(vec![Value::from("a"), Value::from("b"), Value::from("c")])
    );
}

// ---------------------------------------------------------------------------
// List containing Null
// ---------------------------------------------------------------------------

#[test]
fn eval_list_with_null() {
    assert_eq!(
        Expr::list([Expr::from(Value::Null), Expr::from(1i64)])
            .eval_const()
            .unwrap(),
        Value::List(vec![Value::Null, Value::I64(1)])
    );
}

// ---------------------------------------------------------------------------
// Nested expressions in list (e.g. is_null inside list)
// ---------------------------------------------------------------------------

#[test]
fn eval_list_of_is_null_exprs() {
    let expr = Expr::list([Expr::is_null(Value::Null), Expr::is_null(1i64)]);
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::List(vec![Value::Bool(true), Value::Bool(false)])
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::list([1i64, 2i64]);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
