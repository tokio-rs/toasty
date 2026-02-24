use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// Bool
// ---------------------------------------------------------------------------

#[test]
fn eval_value_bool_true() {
    let expr = Expr::Value(Value::Bool(true));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn eval_value_bool_false() {
    let expr = Expr::Value(Value::Bool(false));
    assert_eq!(expr.eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Null
// ---------------------------------------------------------------------------

#[test]
fn eval_value_null() {
    let expr = Expr::Value(Value::Null);
    assert_eq!(expr.eval_const().unwrap(), Value::Null);
}

// ---------------------------------------------------------------------------
// Signed integers
// ---------------------------------------------------------------------------

#[test]
fn eval_value_i8() {
    let expr = Expr::Value(Value::I8(42));
    assert_eq!(expr.eval_const().unwrap(), Value::I8(42));
}

#[test]
fn eval_value_i8_negative() {
    let expr = Expr::Value(Value::I8(-1));
    assert_eq!(expr.eval_const().unwrap(), Value::I8(-1));
}

#[test]
fn eval_value_i16() {
    let expr = Expr::Value(Value::I16(1000));
    assert_eq!(expr.eval_const().unwrap(), Value::I16(1000));
}

#[test]
fn eval_value_i32() {
    let expr = Expr::Value(Value::I32(100_000));
    assert_eq!(expr.eval_const().unwrap(), Value::I32(100_000));
}

#[test]
fn eval_value_i64() {
    let expr = Expr::Value(Value::I64(9_000_000_000));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(9_000_000_000));
}

#[test]
fn eval_value_i64_zero() {
    let expr = Expr::Value(Value::I64(0));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(0));
}

#[test]
fn eval_value_i64_min() {
    let expr = Expr::Value(Value::I64(i64::MIN));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(i64::MIN));
}

#[test]
fn eval_value_i64_max() {
    let expr = Expr::Value(Value::I64(i64::MAX));
    assert_eq!(expr.eval_const().unwrap(), Value::I64(i64::MAX));
}

// ---------------------------------------------------------------------------
// Unsigned integers
// ---------------------------------------------------------------------------

#[test]
fn eval_value_u8() {
    let expr = Expr::Value(Value::U8(255));
    assert_eq!(expr.eval_const().unwrap(), Value::U8(255));
}

#[test]
fn eval_value_u16() {
    let expr = Expr::Value(Value::U16(65535));
    assert_eq!(expr.eval_const().unwrap(), Value::U16(65535));
}

#[test]
fn eval_value_u32() {
    let expr = Expr::Value(Value::U32(4_294_967_295));
    assert_eq!(expr.eval_const().unwrap(), Value::U32(4_294_967_295));
}

#[test]
fn eval_value_u64() {
    let expr = Expr::Value(Value::U64(u64::MAX));
    assert_eq!(expr.eval_const().unwrap(), Value::U64(u64::MAX));
}

#[test]
fn eval_value_u64_zero() {
    let expr = Expr::Value(Value::U64(0));
    assert_eq!(expr.eval_const().unwrap(), Value::U64(0));
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

#[test]
fn eval_value_string() {
    let expr = Expr::Value(Value::from("hello"));
    assert_eq!(expr.eval_const().unwrap(), Value::from("hello"));
}

#[test]
fn eval_value_string_empty() {
    let expr = Expr::Value(Value::from(""));
    assert_eq!(expr.eval_const().unwrap(), Value::from(""));
}

// ---------------------------------------------------------------------------
// Bytes
// ---------------------------------------------------------------------------

#[test]
fn eval_value_bytes() {
    let expr = Expr::Value(Value::Bytes(vec![1, 2, 3]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bytes(vec![1, 2, 3]));
}

#[test]
fn eval_value_bytes_empty() {
    let expr = Expr::Value(Value::Bytes(vec![]));
    assert_eq!(expr.eval_const().unwrap(), Value::Bytes(vec![]));
}

// ---------------------------------------------------------------------------
// eval() and eval_const() are equivalent for Expr::Value
// ---------------------------------------------------------------------------

#[test]
fn eval_and_eval_const_agree() {
    use toasty_core::stmt::ConstInput;

    let value = Value::I64(99);
    let expr = Expr::Value(value.clone());

    let via_eval = expr.eval(ConstInput::new()).unwrap();
    let via_eval_const = expr.eval_const().unwrap();

    assert_eq!(via_eval, via_eval_const);
    assert_eq!(via_eval, value);
}
