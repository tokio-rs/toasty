use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// Bool
// ---------------------------------------------------------------------------

#[test]
fn eval_value_bool_true() {
    assert_eq!(Expr::from(true).eval_const().unwrap(), Value::Bool(true));
}

#[test]
fn eval_value_bool_false() {
    assert_eq!(Expr::from(false).eval_const().unwrap(), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Null
// ---------------------------------------------------------------------------

#[test]
fn eval_value_null() {
    assert_eq!(Expr::from(Value::Null).eval_const().unwrap(), Value::Null);
}

// ---------------------------------------------------------------------------
// Signed integers
// ---------------------------------------------------------------------------

#[test]
fn eval_value_i8() {
    assert_eq!(
        Expr::from(Value::I8(42)).eval_const().unwrap(),
        Value::I8(42)
    );
}

#[test]
fn eval_value_i8_negative() {
    assert_eq!(
        Expr::from(Value::I8(-1)).eval_const().unwrap(),
        Value::I8(-1)
    );
}

#[test]
fn eval_value_i16() {
    assert_eq!(
        Expr::from(Value::I16(1000)).eval_const().unwrap(),
        Value::I16(1000)
    );
}

#[test]
fn eval_value_i32() {
    assert_eq!(
        Expr::from(Value::I32(100_000)).eval_const().unwrap(),
        Value::I32(100_000)
    );
}

#[test]
fn eval_value_i64() {
    assert_eq!(
        Expr::from(9_000_000_000i64).eval_const().unwrap(),
        Value::I64(9_000_000_000)
    );
}

#[test]
fn eval_value_i64_zero() {
    assert_eq!(Expr::from(0i64).eval_const().unwrap(), Value::I64(0));
}

#[test]
fn eval_value_i64_min() {
    assert_eq!(
        Expr::from(i64::MIN).eval_const().unwrap(),
        Value::I64(i64::MIN)
    );
}

#[test]
fn eval_value_i64_max() {
    assert_eq!(
        Expr::from(i64::MAX).eval_const().unwrap(),
        Value::I64(i64::MAX)
    );
}

// ---------------------------------------------------------------------------
// Unsigned integers
// ---------------------------------------------------------------------------

#[test]
fn eval_value_u8() {
    assert_eq!(
        Expr::from(Value::U8(255)).eval_const().unwrap(),
        Value::U8(255)
    );
}

#[test]
fn eval_value_u16() {
    assert_eq!(
        Expr::from(Value::U16(65535)).eval_const().unwrap(),
        Value::U16(65535)
    );
}

#[test]
fn eval_value_u32() {
    assert_eq!(
        Expr::from(Value::U32(4_294_967_295)).eval_const().unwrap(),
        Value::U32(4_294_967_295)
    );
}

#[test]
fn eval_value_u64() {
    assert_eq!(
        Expr::from(Value::U64(u64::MAX)).eval_const().unwrap(),
        Value::U64(u64::MAX)
    );
}

#[test]
fn eval_value_u64_zero() {
    assert_eq!(
        Expr::from(Value::U64(0)).eval_const().unwrap(),
        Value::U64(0)
    );
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

#[test]
fn eval_value_string() {
    assert_eq!(
        Expr::from("hello").eval_const().unwrap(),
        Value::from("hello")
    );
}

#[test]
fn eval_value_string_empty() {
    assert_eq!(Expr::from("").eval_const().unwrap(), Value::from(""));
}

// ---------------------------------------------------------------------------
// Bytes
// ---------------------------------------------------------------------------

#[test]
fn eval_value_bytes() {
    assert_eq!(
        Expr::from(Value::Bytes(vec![1, 2, 3]))
            .eval_const()
            .unwrap(),
        Value::Bytes(vec![1, 2, 3])
    );
}

#[test]
fn eval_value_bytes_empty() {
    assert_eq!(
        Expr::from(Value::Bytes(vec![])).eval_const().unwrap(),
        Value::Bytes(vec![])
    );
}

// ---------------------------------------------------------------------------
// eval() and eval_const() are equivalent for Expr::Value
// ---------------------------------------------------------------------------

#[test]
fn eval_and_eval_const_agree() {
    use toasty_core::stmt::ConstInput;
    let expr = Expr::from(99i64);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
