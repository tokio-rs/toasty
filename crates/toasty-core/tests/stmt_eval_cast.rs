use toasty_core::stmt::{ConstInput, Expr, Type, Value};

// ---------------------------------------------------------------------------
// Integer widening (i8 → i64)
// ---------------------------------------------------------------------------

#[test]
fn cast_i8_to_i64() {
    assert_eq!(
        Expr::cast(Value::I8(42), Type::I64).eval_const().unwrap(),
        Value::I64(42)
    );
}

#[test]
fn cast_i32_to_i64() {
    assert_eq!(
        Expr::cast(Value::I32(100_000), Type::I64).eval_const().unwrap(),
        Value::I64(100_000)
    );
}

// ---------------------------------------------------------------------------
// Integer narrowing — in range
// ---------------------------------------------------------------------------

#[test]
fn cast_i64_to_i32_in_range() {
    assert_eq!(
        Expr::cast(Value::I64(42), Type::I32).eval_const().unwrap(),
        Value::I32(42)
    );
}

#[test]
fn cast_i64_to_i8_in_range() {
    assert_eq!(
        Expr::cast(Value::I64(100), Type::I8).eval_const().unwrap(),
        Value::I8(100)
    );
}

// ---------------------------------------------------------------------------
// Integer narrowing — out of range → Err
// ---------------------------------------------------------------------------

#[test]
fn cast_i64_to_i8_out_of_range() {
    assert!(Expr::cast(Value::I64(200), Type::I8).eval_const().is_err());
}

#[test]
fn cast_i64_to_i32_out_of_range() {
    assert!(Expr::cast(Value::I64(3_000_000_000), Type::I32).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// Unsigned integer casts
// ---------------------------------------------------------------------------

#[test]
fn cast_u32_to_u64() {
    assert_eq!(
        Expr::cast(Value::U32(255), Type::U64).eval_const().unwrap(),
        Value::U64(255)
    );
}

#[test]
fn cast_u64_to_u8_in_range() {
    assert_eq!(
        Expr::cast(Value::U64(200), Type::U8).eval_const().unwrap(),
        Value::U8(200)
    );
}

#[test]
fn cast_u64_to_u8_out_of_range() {
    assert!(Expr::cast(Value::U64(300), Type::U8).eval_const().is_err());
}

// ---------------------------------------------------------------------------
// String identity cast
// ---------------------------------------------------------------------------

#[test]
fn cast_string_to_string() {
    assert_eq!(
        Expr::cast(Value::from("hello"), Type::String).eval_const().unwrap(),
        Value::from("hello")
    );
}

// ---------------------------------------------------------------------------
// Uuid ↔ String
// ---------------------------------------------------------------------------

#[test]
fn cast_uuid_to_string() {
    let id = uuid::Uuid::nil();
    assert_eq!(
        Expr::cast(Value::Uuid(id), Type::String).eval_const().unwrap(),
        Value::from(id.to_string().as_str())
    );
}

#[test]
fn cast_string_to_uuid() {
    let id = uuid::Uuid::nil();
    assert_eq!(
        Expr::cast(Value::from(id.to_string().as_str()), Type::Uuid)
            .eval_const()
            .unwrap(),
        Value::Uuid(id)
    );
}

// ---------------------------------------------------------------------------
// Null is always passed through unchanged
// ---------------------------------------------------------------------------

#[test]
fn cast_null_to_i64_passes_through() {
    assert_eq!(
        Expr::cast(Value::Null, Type::I64).eval_const().unwrap(),
        Value::Null
    );
}

#[test]
fn cast_null_to_string_passes_through() {
    assert_eq!(
        Expr::cast(Value::Null, Type::String).eval_const().unwrap(),
        Value::Null
    );
}

// ---------------------------------------------------------------------------
// eval() with ConstInput agrees with eval_const()
// ---------------------------------------------------------------------------

#[test]
fn eval_with_input_agrees() {
    let expr = Expr::cast(Value::I8(10), Type::I64);
    assert_eq!(
        expr.eval(ConstInput::new()).unwrap(),
        expr.eval_const().unwrap()
    );
}
