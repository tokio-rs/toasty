use toasty_core::driver::StorageTypes;
use toasty_core::schema::db;
use toasty_core::stmt::{Value, ValueObject, ValueRecord};

// `infer_db_ty` maps a `Value` straight to its storage type, resolving
// string/uuid/bytes/decimal/temporal variants through the driver's defaults.
// These tests pin the SQLite defaults and, in particular, guard that a list of
// distinct value variants is rejected even when those variants happen to share
// one backend storage type.

// ---------------------------------------------------------------------------
// Scalars
// ---------------------------------------------------------------------------

#[test]
fn scalar_resolves_through_storage_defaults() {
    let st = &StorageTypes::SQLITE;
    assert_eq!(Value::I64(1).infer_db_ty(st).unwrap(), db::Type::Integer(8));
    assert_eq!(
        Value::Bool(true).infer_db_ty(st).unwrap(),
        db::Type::Boolean
    );
    // SQLite has no native UUID/bytes types; both store as BLOB.
    assert_eq!(
        Value::Uuid(uuid::Uuid::nil()).infer_db_ty(st).unwrap(),
        db::Type::Blob
    );
    assert_eq!(
        Value::Bytes(vec![1]).infer_db_ty(st).unwrap(),
        db::Type::Blob
    );
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

#[test]
fn uniform_list_infers_element_array_type() {
    let st = &StorageTypes::SQLITE;
    let v = Value::List(vec![Value::I64(1), Value::I64(2)]);
    assert_eq!(
        v.infer_db_ty(st).unwrap(),
        db::Type::List(Box::new(db::Type::Integer(8)))
    );
}

#[test]
fn nested_uniform_list_infers_nested_array_type() {
    let st = &StorageTypes::SQLITE;
    let v = Value::List(vec![Value::List(vec![Value::Bool(true)])]);
    assert_eq!(
        v.infer_db_ty(st).unwrap(),
        db::Type::List(Box::new(db::Type::List(Box::new(db::Type::Boolean))))
    );
}

#[test]
fn mixed_list_is_rejected() {
    // I64 and String never share a storage type.
    let st = &StorageTypes::SQLITE;
    let v = Value::List(vec![Value::I64(1), Value::String("x".into())]);
    let err = v.infer_db_ty(st).unwrap_err();
    assert!(
        err.to_string()
            .contains("is not supported by this database"),
        "got: {err}"
    );
}

#[test]
fn mixed_list_sharing_a_storage_type_is_still_rejected() {
    // Regression: on SQLite both UUID and bytes store as BLOB. The element
    // types differ at the app level, so the list must be rejected — inferring
    // off storage-type equality alone would wrongly accept it as List(Blob).
    let st = &StorageTypes::SQLITE;
    let v = Value::List(vec![
        Value::Uuid(uuid::Uuid::nil()),
        Value::Bytes(vec![1, 2, 3]),
    ]);
    assert!(v.infer_db_ty(st).is_err());
}

#[cfg(feature = "jiff")]
#[test]
fn mixed_string_and_date_sharing_text_is_rejected() {
    // The motivating case: on SQLite, String and Date both store as TEXT.
    let st = &StorageTypes::SQLITE;
    let v = Value::List(vec![
        Value::String("x".into()),
        Value::Date(jiff::civil::date(2025, 1, 1)),
    ]);
    assert!(v.infer_db_ty(st).is_err());
}

#[test]
fn empty_list_is_rejected() {
    let st = &StorageTypes::SQLITE;
    assert!(Value::List(vec![]).infer_db_ty(st).is_err());
}

// ---------------------------------------------------------------------------
// Non-inferable values
// ---------------------------------------------------------------------------

#[test]
fn null_record_and_object_are_rejected() {
    let st = &StorageTypes::SQLITE;
    assert!(Value::Null.infer_db_ty(st).is_err());

    let record = Value::Record(ValueRecord::from_vec(vec![Value::I64(1)]));
    assert!(record.infer_db_ty(st).is_err());

    let object = Value::Object(ValueObject::from_vec(vec![("field".into(), Value::I64(1))]));
    assert!(object.infer_db_ty(st).is_err());
}

#[test]
fn inference_errors_do_not_include_value_data() {
    let value = Value::Record(ValueRecord::from_vec(vec![Value::String(
        "sensitive bind value".into(),
    )]));
    let err = value.infer_db_ty(&StorageTypes::SQLITE).unwrap_err();

    assert!(!err.to_string().contains("sensitive bind value"));
}
