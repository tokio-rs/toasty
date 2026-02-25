use toasty_core::stmt::{Type, Value, ValueRecord};

// ---------------------------------------------------------------------------
// Scalar primitives
// ---------------------------------------------------------------------------

#[test]
fn infer_bool() {
    assert_eq!(Value::Bool(true).infer_ty(), Type::Bool);
    assert_eq!(Value::Bool(false).infer_ty(), Type::Bool);
}

#[test]
fn infer_i8() {
    assert_eq!(Value::I8(0).infer_ty(), Type::I8);
    assert_eq!(Value::I8(i8::MAX).infer_ty(), Type::I8);
    assert_eq!(Value::I8(i8::MIN).infer_ty(), Type::I8);
}

#[test]
fn infer_i16() {
    assert_eq!(Value::I16(1000).infer_ty(), Type::I16);
}

#[test]
fn infer_i32() {
    assert_eq!(Value::I32(100_000).infer_ty(), Type::I32);
}

#[test]
fn infer_i64() {
    assert_eq!(Value::I64(i64::MAX).infer_ty(), Type::I64);
    assert_eq!(Value::I64(-1).infer_ty(), Type::I64);
}

#[test]
fn infer_u8() {
    assert_eq!(Value::U8(255).infer_ty(), Type::U8);
}

#[test]
fn infer_u16() {
    assert_eq!(Value::U16(1000).infer_ty(), Type::U16);
}

#[test]
fn infer_u32() {
    assert_eq!(Value::U32(100_000).infer_ty(), Type::U32);
}

#[test]
fn infer_u64() {
    assert_eq!(Value::U64(u64::MAX).infer_ty(), Type::U64);
}

#[test]
fn infer_string() {
    assert_eq!(Value::String("hello".into()).infer_ty(), Type::String);
    assert_eq!(Value::String(String::new()).infer_ty(), Type::String);
}

#[test]
fn infer_bytes() {
    assert_eq!(Value::Bytes(vec![1, 2, 3]).infer_ty(), Type::Bytes);
    assert_eq!(Value::Bytes(vec![]).infer_ty(), Type::Bytes);
}

#[test]
fn infer_uuid() {
    let id = uuid::Uuid::new_v4();
    assert_eq!(Value::Uuid(id).infer_ty(), Type::Uuid);
}

// ---------------------------------------------------------------------------
// Null
// ---------------------------------------------------------------------------

#[test]
fn infer_null() {
    assert_eq!(Value::Null.infer_ty(), Type::Null);
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

#[test]
fn infer_list_empty_yields_null_element() {
    // An empty list has no elements to inspect, so the element type is Null.
    assert_eq!(Value::List(vec![]).infer_ty(), Type::list(Type::Null));
}

#[test]
fn infer_list_bool() {
    let v = Value::List(vec![Value::Bool(true), Value::Bool(false)]);
    assert_eq!(v.infer_ty(), Type::list(Type::Bool));
}

#[test]
fn infer_list_i32() {
    let v = Value::List(vec![Value::I32(1), Value::I32(2), Value::I32(3)]);
    assert_eq!(v.infer_ty(), Type::list(Type::I32));
}

#[test]
fn infer_list_string() {
    let v = Value::List(vec![Value::String("a".into())]);
    assert_eq!(v.infer_ty(), Type::list(Type::String));
}

#[test]
fn infer_list_uses_first_element() {
    // Type is inferred from the first element only.
    let v = Value::List(vec![Value::I64(10), Value::I64(20)]);
    assert_eq!(v.infer_ty(), Type::list(Type::I64));
}

#[test]
fn infer_list_of_lists() {
    let inner = Value::List(vec![Value::Bool(true)]);
    let v = Value::List(vec![inner]);
    assert_eq!(v.infer_ty(), Type::list(Type::list(Type::Bool)));
}

// ---------------------------------------------------------------------------
// Record
// ---------------------------------------------------------------------------

#[test]
fn infer_record_single_field() {
    let r = ValueRecord::from_vec(vec![Value::I32(42)]);
    assert_eq!(Value::Record(r).infer_ty(), Type::Record(vec![Type::I32]));
}

#[test]
fn infer_record_mixed_fields() {
    let r = ValueRecord::from_vec(vec![
        Value::I32(1),
        Value::String("x".into()),
        Value::Bool(true),
    ]);
    assert_eq!(
        Value::Record(r).infer_ty(),
        Type::Record(vec![Type::I32, Type::String, Type::Bool])
    );
}

#[test]
fn infer_record_with_null_field() {
    let r = ValueRecord::from_vec(vec![Value::I64(5), Value::Null]);
    assert_eq!(
        Value::Record(r).infer_ty(),
        Type::Record(vec![Type::I64, Type::Null])
    );
}

#[test]
fn infer_nested_record() {
    let inner = ValueRecord::from_vec(vec![Value::Bool(false), Value::U32(7)]);
    let outer = ValueRecord::from_vec(vec![Value::String("top".into()), Value::Record(inner)]);
    assert_eq!(
        Value::Record(outer).infer_ty(),
        Type::Record(vec![
            Type::String,
            Type::Record(vec![Type::Bool, Type::U32])
        ])
    );
}
