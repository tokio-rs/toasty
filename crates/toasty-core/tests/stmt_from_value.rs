use toasty_core::stmt::Value;

// ---------------------------------------------------------------------------
// bool
// ---------------------------------------------------------------------------

#[test]
fn from_bool_true() {
    assert_eq!(Value::from(true), Value::Bool(true));
}

#[test]
fn from_bool_false() {
    assert_eq!(Value::from(false), Value::Bool(false));
}

// ---------------------------------------------------------------------------
// Signed integers
// ---------------------------------------------------------------------------

#[test]
fn from_i8() {
    assert_eq!(Value::from(42i8), Value::I8(42));
}

#[test]
fn from_ref_i8() {
    assert_eq!(Value::from(&42i8), Value::I8(42));
}

#[test]
fn from_i16() {
    assert_eq!(Value::from(1000i16), Value::I16(1000));
}

#[test]
fn from_ref_i16() {
    assert_eq!(Value::from(&1000i16), Value::I16(1000));
}

#[test]
fn from_i32() {
    assert_eq!(Value::from(100_000i32), Value::I32(100_000));
}

#[test]
fn from_ref_i32() {
    assert_eq!(Value::from(&100_000i32), Value::I32(100_000));
}

#[test]
fn from_i64() {
    assert_eq!(Value::from(9_000_000_000i64), Value::I64(9_000_000_000));
}

#[test]
fn from_ref_i64() {
    assert_eq!(Value::from(&9_000_000_000i64), Value::I64(9_000_000_000));
}

// ---------------------------------------------------------------------------
// Unsigned integers
// ---------------------------------------------------------------------------

#[test]
fn from_u8() {
    assert_eq!(Value::from(255u8), Value::U8(255));
}

#[test]
fn from_ref_u8() {
    assert_eq!(Value::from(&255u8), Value::U8(255));
}

#[test]
fn from_u16() {
    assert_eq!(Value::from(65535u16), Value::U16(65535));
}

#[test]
fn from_ref_u16() {
    assert_eq!(Value::from(&65535u16), Value::U16(65535));
}

#[test]
fn from_u32() {
    assert_eq!(Value::from(4_294_967_295u32), Value::U32(4_294_967_295));
}

#[test]
fn from_ref_u32() {
    assert_eq!(Value::from(&4_294_967_295u32), Value::U32(4_294_967_295));
}

#[test]
fn from_u64() {
    assert_eq!(Value::from(u64::MAX), Value::U64(u64::MAX));
}

#[test]
fn from_ref_u64() {
    assert_eq!(Value::from(&u64::MAX), Value::U64(u64::MAX));
}

// ---------------------------------------------------------------------------
// Platform-sized integers (widen to U64 / I64)
// ---------------------------------------------------------------------------

#[test]
fn from_usize() {
    assert_eq!(Value::from(42usize), Value::U64(42));
}

#[test]
fn from_ref_usize() {
    assert_eq!(Value::from(&42usize), Value::U64(42));
}

#[test]
fn from_isize() {
    assert_eq!(Value::from(42isize), Value::I64(42));
}

#[test]
fn from_ref_isize() {
    assert_eq!(Value::from(&42isize), Value::I64(42));
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

#[test]
fn from_str() {
    assert_eq!(Value::from("hello"), Value::String("hello".to_owned()));
}

#[test]
fn from_string_owned() {
    assert_eq!(
        Value::from("hello".to_owned()),
        Value::String("hello".to_owned())
    );
}

#[test]
fn from_string_ref() {
    let s = "hello".to_owned();
    assert_eq!(Value::from(&s), Value::String("hello".to_owned()));
}

// ---------------------------------------------------------------------------
// Bytes
// ---------------------------------------------------------------------------

#[test]
fn from_vec_u8() {
    assert_eq!(Value::from(vec![1u8, 2, 3]), Value::Bytes(vec![1, 2, 3]));
}

#[test]
fn from_vec_u8_empty() {
    assert_eq!(Value::from(Vec::<u8>::new()), Value::Bytes(vec![]));
}

// ---------------------------------------------------------------------------
// Option<T> — Some maps through, None becomes Null
// ---------------------------------------------------------------------------

#[test]
fn from_option_some_bool() {
    assert_eq!(Value::from(Some(true)), Value::Bool(true));
}

#[test]
fn from_option_none_bool() {
    assert_eq!(Value::from(None::<bool>), Value::Null);
}

#[test]
fn from_option_some_i64() {
    assert_eq!(Value::from(Some(42i64)), Value::I64(42));
}

#[test]
fn from_option_none_i64() {
    assert_eq!(Value::from(None::<i64>), Value::Null);
}

#[test]
fn from_option_some_str() {
    assert_eq!(
        Value::from(Some("hello")),
        Value::String("hello".to_owned())
    );
}

#[test]
fn from_option_none_str() {
    assert_eq!(Value::from(None::<&str>), Value::Null);
}

// ---------------------------------------------------------------------------
// uuid::Uuid
// ---------------------------------------------------------------------------

#[test]
fn from_uuid_nil() {
    let id = uuid::Uuid::nil();
    assert_eq!(Value::from(id), Value::Uuid(id));
}

#[test]
fn from_uuid_nonzero() {
    let id = uuid::Uuid::from_u128(0x1234_5678_9abc_def0_fedc_ba98_7654_3210);
    assert_eq!(Value::from(id), Value::Uuid(id));
}
