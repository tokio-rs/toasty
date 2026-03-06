use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Identity map — arg(0) returns each item unchanged
// ---------------------------------------------------------------------------

#[test]
fn map_identity_i64() {
    let expr = Expr::map(Expr::list([1i64, 2i64, 3i64]), Expr::arg(0usize));
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::List(vec![Value::I64(1), Value::I64(2), Value::I64(3)])
    );
}

#[test]
fn map_identity_strings() {
    let expr = Expr::map(Expr::list(["a", "b"]), Expr::arg(0usize));
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::List(vec![Value::from("a"), Value::from("b")])
    );
}

// ---------------------------------------------------------------------------
// Empty list maps to empty list
// ---------------------------------------------------------------------------

#[test]
fn map_empty_list() {
    let expr = Expr::map(Expr::list(std::iter::empty::<Expr>()), Expr::arg(0usize));
    assert_eq!(expr.eval_const().unwrap(), Value::List(vec![]));
}

// ---------------------------------------------------------------------------
// Transform map — apply a function to each item
// ---------------------------------------------------------------------------

#[test]
fn map_is_null_over_list() {
    // [Null, I64(1)] → [true, false]
    let expr = Expr::map(
        Expr::list([Expr::from(Value::Null), Expr::from(1i64)]),
        Expr::is_null(Expr::arg(0usize)),
    );
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::List(vec![Value::Bool(true), Value::Bool(false)])
    );
}

#[test]
fn map_not_over_bools() {
    // [true, false] → [false, true]
    let expr = Expr::map(Expr::list([true, false]), Expr::not(Expr::arg(0usize)));
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::List(vec![Value::Bool(false), Value::Bool(true)])
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::map(Expr::list([1i64]), Expr::arg(0usize));
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}

// ---------------------------------------------------------------------------
// Non-list base — returns Err
// ---------------------------------------------------------------------------

#[test]
fn map_non_list_base_is_error() {
    let expr = Expr::map(42i64, Expr::arg(0usize));
    assert!(expr.eval_const().is_err());
}
