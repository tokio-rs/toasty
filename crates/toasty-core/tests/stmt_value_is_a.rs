use toasty_core::stmt::{Type, Value};

// ---------------------------------------------------------------------------
// Value::Null is a member of every type
// ---------------------------------------------------------------------------

#[test]
fn null_is_a_bool() {
    assert!(Value::Null.is_a(&Type::Bool));
}

#[test]
fn null_is_a_string() {
    assert!(Value::Null.is_a(&Type::String));
}

#[test]
fn null_is_a_i64() {
    assert!(Value::Null.is_a(&Type::I64));
}

#[test]
fn null_is_a_uuid() {
    assert!(Value::Null.is_a(&Type::Uuid));
}

#[test]
fn null_is_a_bytes() {
    assert!(Value::Null.is_a(&Type::Bytes));
}

#[test]
fn null_is_a_list() {
    assert!(Value::Null.is_a(&Type::list(Type::String)));
}

#[test]
fn null_is_a_record() {
    assert!(Value::Null.is_a(&Type::Record(vec![Type::I64, Type::String])));
}

// ---------------------------------------------------------------------------
// Scalar values match their own type
// ---------------------------------------------------------------------------

#[test]
fn bool_is_a_bool() {
    assert!(Value::Bool(true).is_a(&Type::Bool));
    assert!(Value::Bool(false).is_a(&Type::Bool));
}

#[test]
fn i8_is_a_i8() {
    assert!(Value::I8(42).is_a(&Type::I8));
}

#[test]
fn i16_is_a_i16() {
    assert!(Value::I16(300).is_a(&Type::I16));
}

#[test]
fn i32_is_a_i32() {
    assert!(Value::I32(-1).is_a(&Type::I32));
}

#[test]
fn i64_is_a_i64() {
    assert!(Value::I64(i64::MAX).is_a(&Type::I64));
}

#[test]
fn u8_is_a_u8() {
    assert!(Value::U8(255).is_a(&Type::U8));
}

#[test]
fn u16_is_a_u16() {
    assert!(Value::U16(1000).is_a(&Type::U16));
}

#[test]
fn u32_is_a_u32() {
    assert!(Value::U32(u32::MAX).is_a(&Type::U32));
}

#[test]
fn u64_is_a_u64() {
    assert!(Value::U64(u64::MAX).is_a(&Type::U64));
}

#[test]
fn string_is_a_string() {
    assert!(Value::from("hello").is_a(&Type::String));
}

#[test]
fn bytes_is_a_bytes() {
    assert!(Value::Bytes(vec![1, 2, 3]).is_a(&Type::Bytes));
}

#[test]
fn uuid_is_a_uuid() {
    assert!(Value::Uuid(uuid::Uuid::nil()).is_a(&Type::Uuid));
}

// ---------------------------------------------------------------------------
// Scalar values do NOT match a different type
// ---------------------------------------------------------------------------

#[test]
fn bool_not_a_string() {
    assert!(!Value::Bool(true).is_a(&Type::String));
}

#[test]
fn bool_not_a_i64() {
    assert!(!Value::Bool(true).is_a(&Type::I64));
}

#[test]
fn i8_not_a_i16() {
    assert!(!Value::I8(1).is_a(&Type::I16));
}

#[test]
fn i32_not_a_i64() {
    assert!(!Value::I32(1).is_a(&Type::I64));
}

#[test]
fn i64_not_a_u64() {
    assert!(!Value::I64(1).is_a(&Type::U64));
}

#[test]
fn string_not_a_bytes() {
    assert!(!Value::from("hi").is_a(&Type::Bytes));
}

#[test]
fn string_not_a_bool() {
    assert!(!Value::from("true").is_a(&Type::Bool));
}

#[test]
fn bytes_not_a_string() {
    assert!(!Value::Bytes(vec![]).is_a(&Type::String));
}

#[test]
fn uuid_not_a_string() {
    assert!(!Value::Uuid(uuid::Uuid::nil()).is_a(&Type::String));
}

#[test]
fn scalar_not_a_list() {
    assert!(!Value::I64(1).is_a(&Type::list(Type::I64)));
}

#[test]
fn scalar_not_a_record() {
    assert!(!Value::I64(1).is_a(&Type::Record(vec![Type::I64])));
}

// ---------------------------------------------------------------------------
// List: empty matches any List<T>
// ---------------------------------------------------------------------------

#[test]
fn empty_list_is_a_list_string() {
    assert!(Value::List(vec![]).is_a(&Type::list(Type::String)));
}

#[test]
fn empty_list_is_a_list_i64() {
    assert!(Value::List(vec![]).is_a(&Type::list(Type::I64)));
}

#[test]
fn empty_list_is_a_list_bool() {
    assert!(Value::List(vec![]).is_a(&Type::list(Type::Bool)));
}

// ---------------------------------------------------------------------------
// List: non-empty checks first element's type
// ---------------------------------------------------------------------------

#[test]
fn list_i64_is_a_list_i64() {
    let list = Value::List(vec![Value::I64(1), Value::I64(2)]);
    assert!(list.is_a(&Type::list(Type::I64)));
}

#[test]
fn list_string_is_a_list_string() {
    let list = Value::List(vec![Value::from("a"), Value::from("b")]);
    assert!(list.is_a(&Type::list(Type::String)));
}

#[test]
fn list_i64_not_a_list_string() {
    let list = Value::List(vec![Value::I64(1), Value::I64(2)]);
    assert!(!list.is_a(&Type::list(Type::String)));
}

#[test]
fn list_not_a_scalar() {
    let list = Value::List(vec![Value::I64(1)]);
    assert!(!list.is_a(&Type::I64));
}

#[test]
fn list_not_a_record() {
    let list = Value::List(vec![Value::I64(1)]);
    assert!(!list.is_a(&Type::Record(vec![Type::I64])));
}

// ---------------------------------------------------------------------------
// Record: matching length and field types
// ---------------------------------------------------------------------------

#[test]
fn record_matches_exact_field_types() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::from("hi")]);
    assert!(rec.is_a(&Type::Record(vec![Type::I64, Type::String])));
}

#[test]
fn record_three_fields() {
    let rec = Value::record_from_vec(vec![Value::Bool(true), Value::I32(5), Value::Bytes(vec![])]);
    assert!(rec.is_a(&Type::Record(vec![Type::Bool, Type::I32, Type::Bytes])));
}

#[test]
fn record_wrong_field_type() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::from("hi")]);
    assert!(!rec.is_a(&Type::Record(vec![Type::String, Type::String])));
}

#[test]
fn record_too_short() {
    let rec = Value::record_from_vec(vec![Value::I64(1)]);
    assert!(!rec.is_a(&Type::Record(vec![Type::I64, Type::String])));
}

#[test]
fn record_too_long() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::I64(2), Value::I64(3)]);
    assert!(!rec.is_a(&Type::Record(vec![Type::I64, Type::I64])));
}

#[test]
fn record_not_a_list() {
    let rec = Value::record_from_vec(vec![Value::I64(1)]);
    assert!(!rec.is_a(&Type::list(Type::I64)));
}

#[test]
fn record_not_a_scalar() {
    let rec = Value::record_from_vec(vec![Value::I64(1)]);
    assert!(!rec.is_a(&Type::I64));
}

// ---------------------------------------------------------------------------
// Record with Null fields — Null is_a any field type
// ---------------------------------------------------------------------------

#[test]
fn record_null_field_matches_any_type() {
    let rec = Value::record_from_vec(vec![Value::I64(1), Value::Null]);
    // Null in field 1 matches Type::String
    assert!(rec.is_a(&Type::Record(vec![Type::I64, Type::String])));
    // Null in field 1 also matches Type::Bool
    assert!(rec.is_a(&Type::Record(vec![Type::I64, Type::Bool])));
}

#[test]
fn record_all_null_fields() {
    let rec = Value::record_from_vec(vec![Value::Null, Value::Null]);
    assert!(rec.is_a(&Type::Record(vec![Type::I64, Type::String])));
    assert!(rec.is_a(&Type::Record(vec![Type::Bool, Type::Uuid])));
}

// ---------------------------------------------------------------------------
// List with Null elements — Null first element matches any List<T>
// ---------------------------------------------------------------------------

#[test]
fn list_with_null_first_is_a_list_string() {
    let list = Value::List(vec![Value::Null, Value::Null]);
    assert!(list.is_a(&Type::list(Type::String)));
}

#[test]
fn list_with_null_first_is_a_list_i64() {
    let list = Value::List(vec![Value::Null]);
    assert!(list.is_a(&Type::list(Type::I64)));
}
