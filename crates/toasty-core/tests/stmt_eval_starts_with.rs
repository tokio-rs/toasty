use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// Prefix matches → true
// ---------------------------------------------------------------------------

#[test]
fn starts_with_matching_prefix() {
    let expr = Expr::starts_with("hello", "he");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn starts_with_full_string() {
    let expr = Expr::starts_with("hello", "hello");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn starts_with_empty_prefix() {
    let expr = Expr::starts_with("hello", "");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Prefix does not match → false
// ---------------------------------------------------------------------------

#[test]
fn starts_with_non_matching_prefix() {
    let expr = Expr::starts_with("hello", "lo");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn starts_with_prefix_longer_than_subject() {
    let expr = Expr::starts_with("he", "hello");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// NULL subject → false (a missing attribute never matches a prefix)
// ---------------------------------------------------------------------------

#[test]
fn starts_with_null_subject_is_false() {
    let expr = Expr::starts_with(Value::Null, "he");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Errors: non-string operands
// ---------------------------------------------------------------------------

#[test]
fn starts_with_non_string_subject_is_error() {
    let expr = Expr::starts_with(42i64, "he");
    assert!(expr.eval_const().is_err());
}

#[test]
fn starts_with_non_string_prefix_is_error() {
    let expr = Expr::starts_with("hello", 42i64);
    assert!(expr.eval_const().is_err());
}
