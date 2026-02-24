use toasty_core::stmt::{ConstInput, Expr, ExprArg, Value};

// ---------------------------------------------------------------------------
// Nesting overflow — nesting deeper than the actual scope stack
// ---------------------------------------------------------------------------

#[test]
fn arg_nesting_overflow_is_error() {
    // nesting=1 at top level: the scope walk overshoots Root
    let expr = Expr::Arg(ExprArg {
        position: 0,
        nesting: 1,
    });
    assert!(expr.eval_const().is_err());
}

// ---------------------------------------------------------------------------
// eval_const — no input, so Arg always errors
// ---------------------------------------------------------------------------

#[test]
fn arg_eval_const_is_error() {
    assert!(Expr::arg(0usize).eval_const().is_err());
}

#[test]
fn arg_any_position_eval_const_is_error() {
    assert!(Expr::arg(5usize).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// eval() with ConstInput — same as eval_const (ConstInput resolves nothing)
// ---------------------------------------------------------------------------

#[test]
fn arg_with_const_input_is_error() {
    assert!(Expr::arg(0usize).eval(ConstInput::new()).is_err());
}

// ---------------------------------------------------------------------------
// eval() with Vec<Value> — resolves by position
// ---------------------------------------------------------------------------

#[test]
fn arg_position_0_from_vec() {
    let args = vec![Value::I64(42)];
    assert_eq!(Expr::arg(0usize).eval(&args).unwrap(), Value::I64(42));
}

#[test]
fn arg_position_1_from_vec() {
    let args = vec![Value::I64(1), Value::from("hello")];
    assert_eq!(Expr::arg(1usize).eval(&args).unwrap(), Value::from("hello"));
}

#[test]
fn arg_null_from_vec() {
    let args = vec![Value::Null];
    assert_eq!(Expr::arg(0usize).eval(&args).unwrap(), Value::Null);
}

#[test]
fn arg_bool_from_vec() {
    let args = vec![Value::Bool(true)];
    assert_eq!(Expr::arg(0usize).eval(&args).unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------
// Arg used inside a larger expression with Vec<Value> input
// ---------------------------------------------------------------------------

#[test]
fn arg_inside_is_null() {
    let args = vec![Value::Null];
    let expr = Expr::is_null(Expr::arg(0usize));
    assert_eq!(expr.eval(&args).unwrap(), Value::Bool(true));
}

#[test]
fn arg_inside_not() {
    let args = vec![Value::Bool(false)];
    let expr = Expr::not(Expr::arg(0usize));
    assert_eq!(expr.eval(&args).unwrap(), Value::Bool(true));
}

#[test]
fn arg_inside_binary_op() {
    use toasty_core::stmt::BinaryOp;
    let args = vec![Value::I64(10)];
    let expr = Expr::binary_op(Expr::arg(0usize), BinaryOp::Eq, 10i64);
    assert_eq!(expr.eval(&args).unwrap(), Value::Bool(true));
}
