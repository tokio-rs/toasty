//! Integer decoding from stored JSON documents is checked, not wrapping: a
//! stored value outside the target type's range is a decode error naming the
//! mismatch, never a silently wrapped value.

use toasty_core::schema::app;
use toasty_core::stmt::{Type, Value};
use toasty_sql::json;

fn schema() -> app::Schema {
    app::Schema::default()
}

#[test]
fn in_range_ints_decode() {
    let value = json::list_from_str(&schema(), "[0, 127, -128]", &Type::I8).unwrap();
    assert_eq!(
        value,
        Value::List(vec![Value::I8(0), Value::I8(127), Value::I8(-128)])
    );
}

#[test]
fn negative_int_for_unsigned_type_is_decode_error() {
    let err = json::list_from_str(&schema(), "[-1]", &Type::U64).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("out of range") && msg.contains("U64"),
        "expected an out-of-range decode error naming the type, got: {msg}"
    );
}

#[test]
fn too_wide_int_is_decode_error() {
    let err = json::list_from_str(&schema(), "[300]", &Type::I8).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("out of range") && msg.contains("I8"),
        "expected an out-of-range decode error naming the type, got: {msg}"
    );
}

#[test]
fn unsigned_above_i64_max_decodes() {
    // serde_json widens to i128 internally, so a u64 above `i64::MAX`
    // round-trips.
    let text = format!("[{}]", u64::MAX);
    let value = json::list_from_str(&schema(), &text, &Type::U64).unwrap();
    assert_eq!(value, Value::List(vec![Value::U64(u64::MAX)]));
}
