use toasty_core::stmt::{ConstInput, Expr, ExprArg, Project, Projection, Value};

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

#[test]
fn project_identity_on_record() {
    let record = Expr::record([Expr::from(1i64), Expr::from(2i64)]);
    let expr = Expr::project(record, Projection::identity());
    assert_eq!(
        expr.eval_const().unwrap(),
        Value::record_from_vec(vec![Value::I64(1), Value::I64(2)])
    );
}

// ---------------------------------------------------------------------------
// Multi-step (nested) projection through record-of-records
// ---------------------------------------------------------------------------

#[test]
fn project_nested_record_multi_step() {
    // outer record: [ inner_record([10, 20]), 99 ]
    let inner = Expr::record([Expr::from(10i64), Expr::from(20i64)]);
    let outer = Expr::record([inner, Expr::from(99i64)]);
    // Path [0, 1]: outer field 0 → inner field 1 → 20
    let expr = Expr::project(outer, Projection::from([0usize, 1]));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(20));
}

#[test]
fn project_deeply_nested_record() {
    // three levels: [[ [42] ]]
    let lvl1 = Expr::record([Expr::from(42i64)]);
    let lvl2 = Expr::record([lvl1]);
    let lvl3 = Expr::record([lvl2]);
    let expr = Expr::project(lvl3, Projection::from([0usize, 0, 0]));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(42));
}

// ---------------------------------------------------------------------------
// Projection::push builds paths incrementally
// ---------------------------------------------------------------------------

#[test]
fn projection_push_identity_to_single() {
    let mut p = Projection::identity();
    p.push(2);
    assert_eq!(p.as_slice(), &[2]);
}

#[test]
fn projection_push_single_to_multi() {
    let mut p = Projection::single(1);
    p.push(3);
    assert_eq!(p.as_slice(), &[1, 3]);
}

#[test]
fn projection_push_multi_grows() {
    let mut p = Projection::from([0usize, 1]);
    p.push(2);
    assert_eq!(p.as_slice(), &[0, 1, 2]);
}

#[test]
fn project_nested_via_push() {
    let inner = Expr::record([Expr::from(7i64), Expr::from(8i64)]);
    let outer = Expr::record([inner, Expr::from(0i64)]);
    let mut proj = Projection::single(0);
    proj.push(1);
    let expr = Expr::project(outer, proj);
    assert_eq!(expr.eval_const().unwrap(), Value::I64(8));
}

// ---------------------------------------------------------------------------
// Projection::from — usize and array conversions
// ---------------------------------------------------------------------------

#[test]
fn projection_from_usize() {
    let p = Projection::from(3usize);
    assert_eq!(p.as_slice(), &[3]);
    assert!(!p.is_identity());
}

#[test]
fn projection_from_empty_slice_is_identity() {
    let p = Projection::from(&[][..]);
    assert!(p.is_identity());
}

#[test]
fn projection_from_array() {
    let p = Projection::from([1usize, 2, 3]);
    assert_eq!(p.as_slice(), &[1, 2, 3]);
}

// ---------------------------------------------------------------------------
// Expr::entry returns None for invalid (non-traversable) paths
// ---------------------------------------------------------------------------

#[test]
fn expr_entry_returns_none_on_scalar_with_step() {
    let scalar = Expr::from(42i64);
    assert!(scalar.entry(&Projection::single(0)).is_none());
}

#[test]
fn expr_entry_returns_none_on_bool_with_step() {
    let expr = Expr::from(true);
    assert!(expr.entry(&Projection::single(0)).is_none());
}

#[test]
fn expr_entry_returns_some_for_identity() {
    let expr = Expr::from(42i64);
    assert!(expr.entry(&Projection::identity()).is_some());
}

// ---------------------------------------------------------------------------
// Project trait on Expr and &Expr
// ---------------------------------------------------------------------------

#[test]
fn project_trait_on_expr() {
    let record = Expr::record([Expr::from(5i64), Expr::from(6i64)]);
    let result = record.project(&Projection::single(1)).unwrap();
    assert_eq!(result.eval_const().unwrap(), Value::I64(6));
}

#[test]
fn project_trait_on_ref_expr() {
    let record = Expr::record([Expr::from(5i64), Expr::from(6i64)]);
    let result = (&record).project(&Projection::single(0)).unwrap();
    assert_eq!(result.eval_const().unwrap(), Value::I64(5));
}

#[test]
fn project_trait_returns_none_for_invalid_path() {
    let scalar = Expr::from(42i64);
    assert!(scalar.project(&Projection::single(0)).is_none());
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
    let args = vec![Value::record_from_vec(vec![
        Value::from("alice"),
        Value::I64(30),
    ])];
    let expr = Expr::arg_project(ExprArg::new(0), Projection::single(0));
    assert_eq!(expr.eval(&args).unwrap(), Value::from("alice"));
}

#[test]
fn project_arg_multi_step() {
    // arg(0) is a record-of-records; path [1, 0]
    let inner = Value::record_from_vec(vec![Value::from("nested"), Value::I64(99)]);
    let outer = Value::record_from_vec(vec![Value::I64(0), inner]);
    let args = vec![outer];
    let expr = Expr::arg_project(ExprArg::new(0), Projection::from([1usize, 0]));
    assert_eq!(expr.eval(&args).unwrap(), Value::from("nested"));
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
