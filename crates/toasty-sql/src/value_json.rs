//! JSON encoding for `stmt::Value`s stored in document-backed columns
//! (MySQL `JSON`, SQLite TEXT via the JSON1 extension, and eventually
//! PostgreSQL `jsonb` for `#[document]`-marked fields). The conversion is
//! intentionally a plain pair of functions rather than a `Serialize` /
//! `Deserialize` impl on `stmt::Value`: the encoding is opinionated
//! (UUIDs / decimals / timestamps as JSON strings) and matches the
//! per-column TEXT encoding the same scalar would have at the SQL level.
//! Backends with typed document storage (BSON, DynamoDB) need different
//! representations.
//!
//! Decoding requires the element type from the schema — `Value::Uuid` vs
//! `Value::String` are both JSON strings on the wire, and only the
//! caller's `stmt::Type` distinguishes them.

use serde_json::Value as Json;
use toasty_core::stmt::{self, Value};

/// Encode a scalar `stmt::Value` as a `serde_json::Value`. Panics on
/// shapes that have no JSON representation (`Record`, nested `List`,
/// `Bytes`, `SparseRecord`, `Null` records, NaN / infinity).
pub fn value_to_json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(v) => Json::Bool(*v),
        Value::I8(v) => Json::Number((*v).into()),
        Value::I16(v) => Json::Number((*v).into()),
        Value::I32(v) => Json::Number((*v).into()),
        Value::I64(v) => Json::Number((*v).into()),
        Value::U8(v) => Json::Number((*v).into()),
        Value::U16(v) => Json::Number((*v).into()),
        Value::U32(v) => Json::Number((*v).into()),
        Value::U64(v) => Json::Number((*v).into()),
        Value::F32(v) => serde_json::Number::from_f64((*v).into())
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::F64(v) => serde_json::Number::from_f64(*v)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::String(v) => Json::String(v.clone()),
        Value::Uuid(v) => Json::String(v.to_string()),
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(v) => Json::String(v.to_string()),
        #[cfg(feature = "bigdecimal")]
        Value::BigDecimal(v) => Json::String(v.to_string()),
        #[cfg(feature = "jiff")]
        Value::Timestamp(v) => Json::String(v.to_string()),
        #[cfg(feature = "jiff")]
        Value::Zoned(v) => Json::String(v.to_string()),
        #[cfg(feature = "jiff")]
        Value::Date(v) => Json::String(v.to_string()),
        #[cfg(feature = "jiff")]
        Value::Time(v) => Json::String(v.to_string()),
        #[cfg(feature = "jiff")]
        Value::DateTime(v) => Json::String(v.to_string()),
        _ => todo!("encode {value:?} as JSON"),
    }
}

/// Decode a `serde_json::Value` into a `stmt::Value` of type `ty`.
pub fn value_from_json(json: Json, ty: &stmt::Type) -> Value {
    match (ty, json) {
        (_, Json::Null) => Value::Null,
        (stmt::Type::Bool, Json::Bool(v)) => Value::Bool(v),
        (stmt::Type::String, Json::String(v)) => Value::String(v),
        (stmt::Type::Uuid, Json::String(v)) => {
            Value::Uuid(v.parse().expect("invalid UUID in JSON"))
        }
        (stmt::Type::I8, Json::Number(n)) => Value::I8(n.as_i64().unwrap() as i8),
        (stmt::Type::I16, Json::Number(n)) => Value::I16(n.as_i64().unwrap() as i16),
        (stmt::Type::I32, Json::Number(n)) => Value::I32(n.as_i64().unwrap() as i32),
        (stmt::Type::I64, Json::Number(n)) => Value::I64(n.as_i64().unwrap()),
        (stmt::Type::U8, Json::Number(n)) => Value::U8(n.as_u64().unwrap() as u8),
        (stmt::Type::U16, Json::Number(n)) => Value::U16(n.as_u64().unwrap() as u16),
        (stmt::Type::U32, Json::Number(n)) => Value::U32(n.as_u64().unwrap() as u32),
        (stmt::Type::U64, Json::Number(n)) => Value::U64(n.as_u64().unwrap()),
        (stmt::Type::F32, Json::Number(n)) => Value::F32(n.as_f64().unwrap() as f32),
        (stmt::Type::F64, Json::Number(n)) => Value::F64(n.as_f64().unwrap()),
        #[cfg(feature = "rust_decimal")]
        (stmt::Type::Decimal, Json::String(v)) => {
            Value::Decimal(v.parse().expect("invalid Decimal in JSON"))
        }
        #[cfg(feature = "bigdecimal")]
        (stmt::Type::BigDecimal, Json::String(v)) => {
            Value::BigDecimal(v.parse().expect("invalid BigDecimal in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Timestamp, Json::String(v)) => {
            Value::Timestamp(v.parse().expect("invalid Timestamp in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Zoned, Json::String(v)) => {
            Value::Zoned(v.parse().expect("invalid Zoned in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Date, Json::String(v)) => {
            Value::Date(v.parse().expect("invalid Date in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Time, Json::String(v)) => {
            Value::Time(v.parse().expect("invalid Time in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::DateTime, Json::String(v)) => {
            Value::DateTime(v.parse().expect("invalid DateTime in JSON"))
        }
        (ty, json) => todo!("decode JSON value {json:?} as {ty:?}"),
    }
}

/// Encode a `Value::List` as a JSON array document. The element type
/// drives per-element encoding via [`value_to_json`].
pub fn value_list_to_json(value: &Value) -> Json {
    let Value::List(items) = value else {
        unreachable!("value_list_to_json called on {value:?}")
    };
    Json::Array(items.iter().map(value_to_json).collect())
}

/// Decode a JSON array document into a `Value::List`, using `elem_ty`
/// as the per-element type.
pub fn value_list_from_json(json: Json, elem_ty: &stmt::Type) -> Value {
    let Json::Array(items) = json else {
        panic!("expected JSON array for Vec<scalar> column, got {json:?}")
    };
    Value::List(
        items
            .into_iter()
            .map(|v| value_from_json(v, elem_ty))
            .collect(),
    )
}
