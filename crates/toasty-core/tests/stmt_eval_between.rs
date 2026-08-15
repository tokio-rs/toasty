use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// Value inside the range → true (bounds are inclusive)
// ---------------------------------------------------------------------------

#[test]
fn between_inside_range() {
    let expr = Expr::between(2i64, 1i64, 3i64);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn between_at_low_bound() {
    let expr = Expr::between(1i64, 1i64, 3i64);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn between_at_high_bound() {
    let expr = Expr::between(3i64, 1i64, 3i64);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn between_strings() {
    let expr = Expr::between("banana", "apple", "cherry");
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Value outside the range → false
// ---------------------------------------------------------------------------

#[test]
fn between_below_range() {
    let expr = Expr::between(0i64, 1i64, 3i64);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn between_above_range() {
    let expr = Expr::between(4i64, 1i64, 3i64);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Errors: NULL operands and incompatible types
// ---------------------------------------------------------------------------

#[test]
fn between_null_subject_is_error() {
    let expr = Expr::between(Value::Null, 1i64, 3i64);
    assert!(expr.eval_const().is_err());
}

#[test]
fn between_incompatible_types_is_error() {
    let expr = Expr::between("banana", 1i64, 3i64);
    assert!(expr.eval_const().is_err());
}
