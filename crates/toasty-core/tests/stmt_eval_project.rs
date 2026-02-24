use toasty_core::stmt::{ConstInput, Expr, ExprArg, Projection, Value};

// ---------------------------------------------------------------------------
// Project from an evaluated record (non-Arg, non-Reference base)
// ---------------------------------------------------------------------------

#[test]
fn project_field_0_from_record() {
    let expr = Expr::project(
        Expr::record([Expr::from(1i64), Expr::from(2i64), Expr::from(3i64)]),
        Projection::single(0),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::I64(1));
}

#[test]
fn project_field_1_from_record() {
    let expr = Expr::project(
        Expr::record([Expr::from(1i64), Expr::from(2i64), Expr::from(3i64)]),
        Projection::single(1),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::I64(2));
}

#[test]
fn project_field_2_from_record() {
    let expr = Expr::project(
        Expr::record([Expr::from(1i64), Expr::from(2i64), Expr::from(3i64)]),
        Projection::single(2),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::I64(3));
}

#[test]
fn project_field_from_mixed_record() {
    let expr = Expr::project(
        Expr::record([Expr::from(true), Expr::from("hello"), Expr::from(42i64)]),
        Projection::single(1),
    );
    assert_eq!(expr.eval_const().unwrap(), Value::from("hello"));
}

// ---------------------------------------------------------------------------
// Project with identity projection — returns the whole value
// ---------------------------------------------------------------------------

#[test]
fn project_identity_returns_value() {
    let expr = Expr::project(Expr::from(99i64), Projection::identity());
    assert_eq!(expr.eval_const().unwrap(), Value::I64(99));
}

// ---------------------------------------------------------------------------
// Project via Arg (ExprArg base path) — requires input
// ---------------------------------------------------------------------------

#[test]
fn project_arg_field_0() {
    // arg(0) points at a record; project field 0
    let args = vec![Value::record_from_vec(vec![Value::I64(10), Value::I64(20)])];
    let expr = Expr::arg_project(ExprArg::new(0), Projection::single(0));
    assert_eq!(expr.eval(&args).unwrap(), Value::I64(10));
}

#[test]
fn project_arg_field_1() {
    let args = vec![Value::record_from_vec(vec![Value::I64(10), Value::I64(20)])];
    let expr = Expr::arg_project(ExprArg::new(0), Projection::single(1));
    assert_eq!(expr.eval(&args).unwrap(), Value::I64(20));
}

#[test]
fn project_arg_string_field() {
    let args = vec![Value::record_from_vec(vec![Value::from("alice"), Value::I64(30)])];
    let expr = Expr::arg_project(ExprArg::new(0), Projection::single(0));
    assert_eq!(expr.eval(&args).unwrap(), Value::from("alice"));
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const() for literal-record base
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::project(
        Expr::record([Expr::from(7i64), Expr::from(8i64)]),
        Projection::single(0),
    );
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
