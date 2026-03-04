use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// is_null — null input
// ---------------------------------------------------------------------------

#[test]
fn is_null_with_null() {
    assert_eq!(
        Expr::is_null(Value::Null).eval_const().unwrap(),
        Value::Bool(true)
    );
}

// ---------------------------------------------------------------------------
// is_null — non-null inputs
// ---------------------------------------------------------------------------

#[test]
fn is_null_with_bool() {
    assert_eq!(
        Expr::is_null(false).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_null_with_i64() {
    assert_eq!(
        Expr::is_null(0i64).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn is_null_with_empty_string() {
    // Empty string is not null.
    assert_eq!(Expr::is_null("").eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn is_null_with_nonempty_string() {
    assert_eq!(
        Expr::is_null("hello").eval_const().unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// is_null — nested: inner expr evaluates to a non-null Bool
// ---------------------------------------------------------------------------

#[test]
fn is_null_of_is_null_result() {
    // is_null(Null) → Bool(true), then is_null(Bool(true)) → false
    assert_eq!(
        Expr::is_null(Expr::is_null(Value::Null))
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// is_not_null — null input
// ---------------------------------------------------------------------------

#[test]
fn is_not_null_with_null() {
    assert_eq!(
        Expr::is_not_null(Value::Null).eval_const().unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// is_not_null — non-null inputs
// ---------------------------------------------------------------------------

#[test]
fn is_not_null_with_i64() {
    assert_eq!(
        Expr::is_not_null(42i64).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn is_not_null_with_string() {
    assert_eq!(
        Expr::is_not_null("hello").eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn is_not_null_with_bool_false() {
    // Bool(false) is not null.
    assert_eq!(
        Expr::is_not_null(false).eval_const().unwrap(),
        Value::Bool(true)
    );
}
