use toasty_core::stmt::{Expr, ExprAnd, Value};

// Build an ExprAnd directly, bypassing Expr::and smart-collapse logic
// (which eliminates operands that are literal `true`).
fn and(operands: Vec<Expr>) -> Expr {
    ExprAnd { operands }.into()
}

// ---------------------------------------------------------------------------
// All-true cases
// ---------------------------------------------------------------------------

#[test]
fn and_two_true_is_true() {
    assert_eq!(
        and(vec![true.into(), true.into()]).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn and_three_true_is_true() {
    assert_eq!(
        and(vec![true.into(), true.into(), true.into()])
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

// ---------------------------------------------------------------------------
// Any-false cases
// ---------------------------------------------------------------------------

#[test]
fn and_first_false_is_false() {
    assert_eq!(
        and(vec![false.into(), true.into()]).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn and_last_false_is_false() {
    assert_eq!(
        and(vec![true.into(), false.into()]).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn and_middle_false_is_false() {
    assert_eq!(
        and(vec![true.into(), false.into(), true.into()])
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn and_all_false_is_false() {
    assert_eq!(
        and(vec![false.into(), false.into()]).eval_const().unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Short-circuit: operands after the first false are not evaluated
// (demonstrated by placing an expression that would error after the false)
// ---------------------------------------------------------------------------

#[test]
fn and_short_circuits_on_false() {
    // not(I64) would error if evaluated, but the false operand comes first.
    let expr = and(vec![false.into(), Expr::not(99i64)]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Composition with other expression kinds
// ---------------------------------------------------------------------------

#[test]
fn and_with_is_null_operands() {
    // is_null(Null) → true, is_null(I64) → false → and → false
    let expr = and(vec![Expr::is_null(Value::Null), Expr::is_null(1i64)]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

#[test]
fn and_with_not_operands() {
    // not(false) → true, not(false) → true → and → true
    let expr = and(vec![Expr::not(false), Expr::not(false)]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Error: non-bool operand
// ---------------------------------------------------------------------------

#[test]
fn and_non_bool_operand_is_error() {
    assert!(and(vec![true.into(), 1i64.into()]).eval_const().is_err());
}
