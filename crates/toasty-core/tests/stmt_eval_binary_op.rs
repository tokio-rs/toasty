use toasty_core::stmt::{BinaryOp, Expr, Value};

// ---------------------------------------------------------------------------
// Eq
// ---------------------------------------------------------------------------

#[test]
fn eq_equal_i64() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Eq, 1i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn eq_different_i64() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Eq, 2i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn eq_equal_strings() {
    assert_eq!(
        Expr::binary_op("hello", BinaryOp::Eq, "hello")
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn eq_different_strings() {
    assert_eq!(
        Expr::binary_op("foo", BinaryOp::Eq, "bar")
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn eq_null_null() {
    assert_eq!(
        Expr::binary_op(Value::Null, BinaryOp::Eq, Value::Null)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn eq_null_nonnull() {
    assert_eq!(
        Expr::binary_op(Value::Null, BinaryOp::Eq, 1i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn eq_bool_true_true() {
    assert_eq!(
        Expr::binary_op(true, BinaryOp::Eq, true)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn eq_bool_true_false() {
    assert_eq!(
        Expr::binary_op(true, BinaryOp::Eq, false)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Ne
// ---------------------------------------------------------------------------

#[test]
fn ne_equal_i64() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Ne, 1i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn ne_different_i64() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Ne, 2i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn ne_null_null() {
    assert_eq!(
        Expr::binary_op(Value::Null, BinaryOp::Ne, Value::Null)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Gt
// ---------------------------------------------------------------------------

#[test]
fn gt_greater() {
    assert_eq!(
        Expr::binary_op(5i64, BinaryOp::Gt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn gt_equal() {
    assert_eq!(
        Expr::binary_op(3i64, BinaryOp::Gt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn gt_less() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Gt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Ge
// ---------------------------------------------------------------------------

#[test]
fn ge_greater() {
    assert_eq!(
        Expr::binary_op(5i64, BinaryOp::Ge, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn ge_equal() {
    assert_eq!(
        Expr::binary_op(3i64, BinaryOp::Ge, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn ge_less() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Ge, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Lt
// ---------------------------------------------------------------------------

#[test]
fn lt_less() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Lt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn lt_equal() {
    assert_eq!(
        Expr::binary_op(3i64, BinaryOp::Lt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn lt_greater() {
    assert_eq!(
        Expr::binary_op(5i64, BinaryOp::Lt, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// Le
// ---------------------------------------------------------------------------

#[test]
fn le_less() {
    assert_eq!(
        Expr::binary_op(1i64, BinaryOp::Le, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn le_equal() {
    assert_eq!(
        Expr::binary_op(3i64, BinaryOp::Le, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn le_greater() {
    assert_eq!(
        Expr::binary_op(5i64, BinaryOp::Le, 3i64)
            .eval_const()
            .unwrap(),
        Value::Bool(false)
    );
}

// ---------------------------------------------------------------------------
// eval() with input agrees with eval_const() for constant operands
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees_with_eval_const() {
    use toasty_core::stmt::ConstInput;
    let expr = Expr::binary_op(1i64, BinaryOp::Eq, 1i64);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
