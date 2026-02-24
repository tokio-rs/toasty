use toasty_core::stmt::{ConstInput, Expr, ExprOr, Value};

// Build an ExprOr directly, bypassing Expr::or smart-collapse logic
// (which eliminates operands that are literal `false`).
fn or(operands: Vec<Expr>) -> Expr {
    ExprOr { operands }.into()
}

// ---------------------------------------------------------------------------
// All-false cases
// ---------------------------------------------------------------------------

#[test]
fn or_two_false_is_false() {
    assert_eq!(
        or(vec![false.into(), false.into()]).eval_const().unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn or_three_false_is_false() {
    assert_eq!(
        or(vec![false.into(), false.into(), false.into()])
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Any-true cases
// ---------------------------------------------------------------------------

#[test]
fn or_first_true_is_true() {
    assert_eq!(
        or(vec![true.into(), false.into()]).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn or_last_true_is_true() {
    assert_eq!(
        or(vec![false.into(), true.into()]).eval_const().unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn or_middle_true_is_true() {
    assert_eq!(
        or(vec![false.into(), true.into(), false.into()])
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn or_all_true_is_true() {
    assert_eq!(
        or(vec![true.into(), true.into()]).eval_const().unwrap(),
        Value::Bool(true)
    );
}

// ---------------------------------------------------------------------------
// Short-circuit: operands after the first true are not evaluated
// ---------------------------------------------------------------------------

#[test]
fn or_short_circuits_on_true() {
    // not(I64) would error if evaluated — but true comes first.
    let error_if_evaled = Expr::not(Expr::from(99i64));
    let expr = or(vec![true.into(), error_if_evaled]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Composition with other expression kinds
// ---------------------------------------------------------------------------

#[test]
fn or_with_is_null_operands() {
    // is_null(I64) → false, is_null(Null) → true → or → true
    let expr = or(vec![Expr::is_null(1i64), Expr::is_null(Value::Null)]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn or_with_not_operands() {
    // not(true) → false, not(true) → false → or → false
    let expr = or(vec![Expr::not(true), Expr::not(true)]);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Error: non-bool operand (including null)
// ---------------------------------------------------------------------------

#[test]
fn or_non_bool_operand_is_error() {
    assert!(or(vec![false.into(), 1i64.into()]).eval_const().is_err());
}

#[test]
fn or_null_operand_is_error() {
    // false OR null → error (null is not a valid boolean)
    assert!(or(vec![false.into(), Expr::from(Value::Null)])
        .eval_const()
        .is_err());
}

#[test]
fn or_short_circuits_before_null() {
    // true OR null → true (null operand is never evaluated)
    assert_eq!(
        or(vec![true.into(), Expr::from(Value::Null)])
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = or(vec![false.into(), true.into()]);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
