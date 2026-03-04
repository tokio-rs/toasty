use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// Basic negation
// ---------------------------------------------------------------------------

#[test]
fn not_true_is_false() {
    assert_eq!(Expr::not(true).eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn not_false_is_true() {
    assert_eq!(Expr::not(false).eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Double negation
// ---------------------------------------------------------------------------

#[test]
fn not_not_true_is_true() {
    assert_eq!(
        Expr::not(Expr::not(true)).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn not_not_false_is_false() {
    assert_eq!(
        Expr::not(Expr::not(false)).eval_const().unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Composition with is_null
// ---------------------------------------------------------------------------

#[test]
fn not_is_null_null() {
    // is_null(Null) → true, not(true) → false
    assert_eq!(
        Expr::not(Expr::is_null(Value::Null)).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn not_is_null_nonnull() {
    // is_null(I64) → false, not(false) → true
    assert_eq!(
        Expr::not(Expr::is_null(1i64)).eval_const().unwrap(),
        Value::Bool(true)
    );
}

// ---------------------------------------------------------------------------
// Error: non-bool value inside not
// ---------------------------------------------------------------------------

#[test]
fn not_non_bool_is_error() {
    assert!(Expr::not(1i64).eval_const().is_err());
}

#[test]
fn not_null_is_error() {
    // Null is not a Bool, so this errors rather than returning Null.
    assert!(Expr::not(Value::Null).eval_const().is_err());
}
