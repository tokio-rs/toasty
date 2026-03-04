use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Empty list → false
// ---------------------------------------------------------------------------

#[test]
fn in_list_empty_is_false() {
    let expr = Expr::in_list(1i64, Expr::list(std::iter::empty::<Expr>()));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Match found → true
// ---------------------------------------------------------------------------

#[test]
fn in_list_found_i64() {
    let expr = Expr::in_list(2i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn in_list_found_first() {
    let expr = Expr::in_list(1i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn in_list_found_last() {
    let expr = Expr::in_list(3i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn in_list_found_string() {
    let expr = Expr::in_list("b", Expr::list(["a", "b", "c"]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// No match → false
// ---------------------------------------------------------------------------

#[test]
fn in_list_not_found_i64() {
    let expr = Expr::in_list(99i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn in_list_not_found_string() {
    let expr = Expr::in_list("z", Expr::list(["a", "b", "c"]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Null in list — uses Rust structural equality (null == null is true)
// ---------------------------------------------------------------------------

#[test]
fn in_list_null_found() {
    // Null is in [Null, I64(1)] → true (Rust PartialEq: Null == Null)
    let expr = Expr::in_list(
        Value::Null,
        Expr::list([Expr::from(Value::Null), Expr::from(1i64)]),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn in_list_null_not_found() {
    // Null is not in [I64(1), I64(2)] → false
    let expr = Expr::in_list(
        Value::Null,
        Expr::list([Expr::from(1i64), Expr::from(2i64)]),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Error: right-hand side is not a list
// ---------------------------------------------------------------------------

#[test]
fn in_list_non_list_rhs_is_error() {
    let expr = Expr::in_list(1i64, Expr::from(1i64));
    assert!(expr.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::in_list(2i64, Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
