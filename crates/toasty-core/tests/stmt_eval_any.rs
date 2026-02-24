use toasty_core::stmt::{ConstInput, Expr, Value};

// ---------------------------------------------------------------------------
// Empty list → false
// ---------------------------------------------------------------------------

#[test]
fn any_empty_list_is_false() {
    let expr = Expr::any(Expr::list(std::iter::empty::<Expr>()));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// All-false list → false
// ---------------------------------------------------------------------------

#[test]
fn any_all_false_is_false() {
    let expr = Expr::any(Expr::list([false, false, false]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Any-true cases
// ---------------------------------------------------------------------------

#[test]
fn any_single_true_is_true() {
    let expr = Expr::any(Expr::list([true]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn any_first_true_is_true() {
    let expr = Expr::any(Expr::list([true, false, false]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn any_last_true_is_true() {
    let expr = Expr::any(Expr::list([false, false, true]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn any_all_true_is_true() {
    let expr = Expr::any(Expr::list([true, true]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Composition with Map — typical usage pattern
// ---------------------------------------------------------------------------

#[test]
fn any_over_map_is_null() {
    // any(map([Null, 1], is_null(arg(0)))) → any([true, false]) → true
    let mapped = Expr::map(
        Expr::list([Expr::from(Value::Null), Expr::from(1i64)]),
        Expr::is_null(Expr::arg(0usize)),
    );
    let expr = Expr::any(mapped);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn any_over_map_all_non_null() {
    // any(map([1, 2], is_null(arg(0)))) → any([false, false]) → false
    let mapped = Expr::map(
        Expr::list([Expr::from(1i64), Expr::from(2i64)]),
        Expr::is_null(Expr::arg(0usize)),
    );
    let expr = Expr::any(mapped);
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Error: inner expression evaluates to a non-list
// ---------------------------------------------------------------------------

#[test]
fn any_non_list_base_is_error() {
    let expr = Expr::any(Expr::from(true));
    assert!(expr.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Error: list item evaluates to a non-bool
// ---------------------------------------------------------------------------

#[test]
fn any_non_bool_item_is_error() {
    let expr = Expr::any(Expr::list([Expr::from(1i64)]));
    assert!(expr.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::any(Expr::list([false, true]));
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
