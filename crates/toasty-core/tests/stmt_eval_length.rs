use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// List operand → element count
// ---------------------------------------------------------------------------

#[test]
fn length_of_list() {
    let expr = Expr::array_length(Expr::list([1i64, 2i64, 3i64]));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(3));
}

#[test]
fn length_of_empty_list() {
    let expr = Expr::array_length(Expr::list(std::iter::empty::<Expr>()));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(0));
}

// ---------------------------------------------------------------------------
// NULL operand → NULL
// ---------------------------------------------------------------------------

#[test]
fn length_of_null_is_null() {
    let expr = Expr::array_length(Value::Null);
    assert_eq!(expr.eval_const().unwrap(), Value::Null);
}

// ---------------------------------------------------------------------------
// Error: non-list operand
// ---------------------------------------------------------------------------

#[test]
fn length_of_non_list_is_error() {
    let expr = Expr::array_length(42i64);
    assert!(expr.eval_const().is_err());
}
