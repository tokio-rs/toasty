use toasty_core::stmt::{Expr, Value};

// ---------------------------------------------------------------------------
// bool
// ---------------------------------------------------------------------------

#[test]
fn from_bool_true() {
    assert_eq!(Expr::from(true), Expr::Value(Value::Bool(true)));
}

#[test]
fn from_bool_false() {
    assert_eq!(Expr::from(false), Expr::Value(Value::Bool(false)));
}

// ---------------------------------------------------------------------------
// i64 (the only integer type with a direct Expr From impl)
// ---------------------------------------------------------------------------

#[test]
fn from_i64() {
    assert_eq!(Expr::from(42i64), Expr::Value(Value::I64(42)));
}

#[test]
fn from_ref_i64() {
    assert_eq!(Expr::from(&42i64), Expr::Value(Value::I64(42)));
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

#[test]
fn from_str() {
    assert_eq!(Expr::from("hello"), Expr::Value(Value::from("hello")));
}

#[test]
fn from_string_owned() {
    assert_eq!(
        Expr::from("hello".to_owned()),
        Expr::Value(Value::from("hello"))
    );
}

#[test]
fn from_string_ref() {
    let s = "hello".to_owned();
    assert_eq!(Expr::from(&s), Expr::Value(Value::from("hello")));
}

// ---------------------------------------------------------------------------
// Value passthrough
// ---------------------------------------------------------------------------

#[test]
fn from_value_bool() {
    assert_eq!(Expr::from(Value::Bool(true)), Expr::Value(Value::Bool(true)));
}

#[test]
fn from_value_null() {
    assert_eq!(Expr::from(Value::Null), Expr::Value(Value::Null));
}

#[test]
fn from_value_i64() {
    assert_eq!(Expr::from(Value::I64(99)), Expr::Value(Value::I64(99)));
}

#[test]
fn from_value_string() {
    assert_eq!(
        Expr::from(Value::from("hi")),
        Expr::Value(Value::from("hi"))
    );
}

#[test]
fn from_value_bytes() {
    assert_eq!(
        Expr::from(Value::Bytes(vec![1, 2])),
        Expr::Value(Value::Bytes(vec![1, 2]))
    );
}
