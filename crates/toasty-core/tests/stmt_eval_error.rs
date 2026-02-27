use toasty_core::stmt::{ConstInput, Expr, MatchArm, Value};

// ---------------------------------------------------------------------------
// Expr::Error — always fails evaluation with the contained message
// ---------------------------------------------------------------------------

#[test]
fn eval_error_is_err() {
    let expr = Expr::error("unexpected value");
    assert!(expr.eval_const().is_err());
}

#[test]
fn eval_error_message_surfaces() {
    let expr = Expr::error("unexpected value from database");
    let err = expr.eval_const().unwrap_err();
    assert!(
        err.to_string().contains("unexpected value from database"),
        "error should contain the message, got: {err}"
    );
}

#[test]
fn eval_error_with_input_is_err() {
    let expr = Expr::error("bad branch");
    assert!(expr.eval(ConstInput::new()).is_err());
}

// ---------------------------------------------------------------------------
// Expr::Error as match else branch
// ---------------------------------------------------------------------------

#[test]
fn match_else_error_fires_on_no_match() {
    let expr = Expr::match_expr(
        Expr::from(99i64),
        vec![MatchArm {
            pattern: Value::from(0i64),
            expr: Expr::from("zero"),
        }],
        Expr::error("unexpected discriminant"),
    );
    let err = expr.eval_const().unwrap_err();
    assert!(
        err.to_string().contains("unexpected discriminant"),
        "got: {err}"
    );
}

#[test]
fn match_else_error_not_reached_when_arm_matches() {
    let expr = Expr::match_expr(
        Expr::from(0i64),
        vec![MatchArm {
            pattern: Value::from(0i64),
            expr: Expr::from("zero"),
        }],
        Expr::error("should not be reached"),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::from("zero"));
}

// ---------------------------------------------------------------------------
// Expr::Error as match arm body
// ---------------------------------------------------------------------------

#[test]
fn match_arm_error_fires_when_matched() {
    let expr = Expr::match_expr(
        Expr::from(1i64),
        vec![
            MatchArm {
                pattern: Value::from(0i64),
                expr: Expr::from("ok"),
            },
            MatchArm {
                pattern: Value::from(1i64),
                expr: Expr::error("arm error"),
            },
        ],
        Expr::from("else"),
    );
    let err = expr.eval_const().unwrap_err();
    assert!(err.to_string().contains("arm error"), "got: {err}");
}

// ---------------------------------------------------------------------------
// eval_bool on error is also Err
// ---------------------------------------------------------------------------

#[test]
fn eval_bool_error_is_err() {
    let expr = Expr::error("not a bool");
    assert!(expr.eval_bool(ConstInput::new()).is_err());
}
