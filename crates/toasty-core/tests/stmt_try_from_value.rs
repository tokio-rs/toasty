use toasty_core::stmt::Value;

// ---------------------------------------------------------------------------
// bool
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_bool_true() {
    assert_eq!(bool::try_from(Value::Bool(true)).unwrap(), true);
}

#[test]
fn try_from_value_bool_false() {
    assert_eq!(bool::try_from(Value::Bool(false)).unwrap(), false);
}

#[test]
fn try_from_value_bool_wrong_type() {
    assert!(bool::try_from(Value::I64(1)).is_err());
}

#[test]
fn try_from_value_bool_null() {
    assert!(bool::try_from(Value::Null).is_err());
}

// ---------------------------------------------------------------------------
// i8
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_i8_same_type() {
    assert_eq!(i8::try_from(Value::I8(42)).unwrap(), 42i8);
}

#[test]
fn try_from_value_i8_negative() {
    assert_eq!(i8::try_from(Value::I8(-1)).unwrap(), -1i8);
}

#[test]
fn try_from_value_i8_cross_type_in_range() {
    // Value::I64 can be losslessly narrowed to i8 when in range.
    assert_eq!(i8::try_from(Value::I64(42)).unwrap(), 42i8);
}

#[test]
fn try_from_value_i8_cross_type_out_of_range() {
    assert!(i8::try_from(Value::I64(200)).is_err());
}

#[test]
fn try_from_value_i8_wrong_type() {
    assert!(i8::try_from(Value::from("hello")).is_err());
}

// ---------------------------------------------------------------------------
// i16
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_i16_same_type() {
    assert_eq!(i16::try_from(Value::I16(1000)).unwrap(), 1000i16);
}

#[test]
fn try_from_value_i16_cross_type_in_range() {
    assert_eq!(i16::try_from(Value::I32(1000)).unwrap(), 1000i16);
}

#[test]
fn try_from_value_i16_cross_type_out_of_range() {
    assert!(i16::try_from(Value::I32(40_000)).is_err());
}

#[test]
fn try_from_value_i16_wrong_type() {
    assert!(i16::try_from(Value::Bool(true)).is_err());
}

// ---------------------------------------------------------------------------
// i32
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_i32_same_type() {
    assert_eq!(i32::try_from(Value::I32(100_000)).unwrap(), 100_000i32);
}

#[test]
fn try_from_value_i32_cross_type_in_range() {
    assert_eq!(i32::try_from(Value::I64(100_000)).unwrap(), 100_000i32);
}

#[test]
fn try_from_value_i32_cross_type_out_of_range() {
    assert!(i32::try_from(Value::I64(3_000_000_000)).is_err());
}

#[test]
fn try_from_value_i32_wrong_type() {
    assert!(i32::try_from(Value::Null).is_err());
}

// ---------------------------------------------------------------------------
// i64
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_i64_same_type() {
    assert_eq!(i64::try_from(Value::I64(9_000_000_000)).unwrap(), 9_000_000_000i64);
}

#[test]
fn try_from_value_i64_cross_type_from_i32() {
    assert_eq!(i64::try_from(Value::I32(42)).unwrap(), 42i64);
}

#[test]
fn try_from_value_i64_cross_type_from_u32() {
    assert_eq!(i64::try_from(Value::U32(u32::MAX)).unwrap(), u32::MAX as i64);
}

#[test]
fn try_from_value_i64_cross_type_out_of_range() {
    // u64::MAX cannot fit in i64.
    assert!(i64::try_from(Value::U64(u64::MAX)).is_err());
}

#[test]
fn try_from_value_i64_wrong_type() {
    assert!(i64::try_from(Value::from("hello")).is_err());
}

// ---------------------------------------------------------------------------
// u8
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_u8_same_type() {
    assert_eq!(u8::try_from(Value::U8(255)).unwrap(), 255u8);
}

#[test]
fn try_from_value_u8_cross_type_in_range() {
    assert_eq!(u8::try_from(Value::U16(200)).unwrap(), 200u8);
}

#[test]
fn try_from_value_u8_cross_type_out_of_range() {
    assert!(u8::try_from(Value::U16(300)).is_err());
}

#[test]
fn try_from_value_u8_negative_is_error() {
    assert!(u8::try_from(Value::I8(-1)).is_err());
}

#[test]
fn try_from_value_u8_wrong_type() {
    assert!(u8::try_from(Value::from("x")).is_err());
}

// ---------------------------------------------------------------------------
// u16
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_u16_same_type() {
    assert_eq!(u16::try_from(Value::U16(65535)).unwrap(), 65535u16);
}

#[test]
fn try_from_value_u16_cross_type_in_range() {
    assert_eq!(u16::try_from(Value::U32(1000)).unwrap(), 1000u16);
}

#[test]
fn try_from_value_u16_cross_type_out_of_range() {
    assert!(u16::try_from(Value::U32(70_000)).is_err());
}

// ---------------------------------------------------------------------------
// u32
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_u32_same_type() {
    assert_eq!(u32::try_from(Value::U32(4_294_967_295)).unwrap(), 4_294_967_295u32);
}

#[test]
fn try_from_value_u32_cross_type_in_range() {
    assert_eq!(u32::try_from(Value::U64(1000)).unwrap(), 1000u32);
}

#[test]
fn try_from_value_u32_cross_type_out_of_range() {
    assert!(u32::try_from(Value::U64(5_000_000_000)).is_err());
}

// ---------------------------------------------------------------------------
// u64
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_u64_same_type() {
    assert_eq!(u64::try_from(Value::U64(u64::MAX)).unwrap(), u64::MAX);
}

#[test]
fn try_from_value_u64_cross_type_from_u32() {
    assert_eq!(u64::try_from(Value::U32(42)).unwrap(), 42u64);
}

#[test]
fn try_from_value_u64_negative_is_error() {
    assert!(u64::try_from(Value::I64(-1)).is_err());
}

#[test]
fn try_from_value_u64_wrong_type() {
    assert!(u64::try_from(Value::Null).is_err());
}

// ---------------------------------------------------------------------------
// usize — widened through u64
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_usize_from_u64() {
    assert_eq!(usize::try_from(Value::U64(42)).unwrap(), 42usize);
}

#[test]
fn try_from_value_usize_cross_type_from_i64_in_range() {
    assert_eq!(usize::try_from(Value::I64(42)).unwrap(), 42usize);
}

#[test]
fn try_from_value_usize_negative_is_error() {
    assert!(usize::try_from(Value::I64(-1)).is_err());
}

#[test]
fn try_from_value_usize_wrong_type() {
    assert!(usize::try_from(Value::from("hello")).is_err());
}

// ---------------------------------------------------------------------------
// isize — widened through i64
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_isize_from_i64() {
    assert_eq!(isize::try_from(Value::I64(42)).unwrap(), 42isize);
}

#[test]
fn try_from_value_isize_cross_type_from_u32() {
    assert_eq!(isize::try_from(Value::U32(42)).unwrap(), 42isize);
}

#[test]
fn try_from_value_isize_wrong_type() {
    assert!(isize::try_from(Value::Null).is_err());
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_string_success() {
    assert_eq!(String::try_from(Value::String("hello".to_owned())).unwrap(), "hello");
}

#[test]
fn try_from_value_string_wrong_type() {
    assert!(String::try_from(Value::I64(42)).is_err());
}

#[test]
fn try_from_value_string_null() {
    assert!(String::try_from(Value::Null).is_err());
}

// ---------------------------------------------------------------------------
// Vec<u8>
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_bytes_success() {
    assert_eq!(Vec::<u8>::try_from(Value::Bytes(vec![1, 2, 3])).unwrap(), vec![1u8, 2, 3]);
}

#[test]
fn try_from_value_bytes_empty() {
    assert_eq!(Vec::<u8>::try_from(Value::Bytes(vec![])).unwrap(), Vec::<u8>::new());
}

#[test]
fn try_from_value_bytes_wrong_type() {
    assert!(Vec::<u8>::try_from(Value::from("hello")).is_err());
}

// ---------------------------------------------------------------------------
// uuid::Uuid
// ---------------------------------------------------------------------------

#[test]
fn try_from_value_uuid_success() {
    let id = uuid::Uuid::nil();
    assert_eq!(uuid::Uuid::try_from(Value::Uuid(id)).unwrap(), id);
}

#[test]
fn try_from_value_uuid_nonzero() {
    let id = uuid::Uuid::from_u128(0x1234_5678_9abc_def0_fedc_ba98_7654_3210);
    assert_eq!(uuid::Uuid::try_from(Value::Uuid(id)).unwrap(), id);
}

#[test]
fn try_from_value_uuid_wrong_type() {
    assert!(uuid::Uuid::try_from(Value::from("not-a-uuid")).is_err());
}
