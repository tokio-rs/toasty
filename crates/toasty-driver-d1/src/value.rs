//! Value conversion between Toasty's `stmt::Value` and the JS values D1
//! binds and returns.
//!
//! D1 moves every number as a JS number (an `f64`) and does not accept
//! `BigInt`, so integers are exact only up to `Number.MAX_SAFE_INTEGER`.

use toasty_core::{
    Error, Result,
    stmt::{self, Value},
};
use worker::{js_sys::Uint8Array, wasm_bindgen::JsValue};

/// Largest integer a JS number holds exactly (`Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// Converts a Toasty value to the JS value bound to a D1 statement.
pub(crate) fn to_js(value: &Value) -> Result<JsValue> {
    Ok(match value {
        Value::Null => JsValue::null(),
        Value::Bool(v) => JsValue::from_bool(*v),
        Value::I8(v) => JsValue::from(*v),
        Value::I16(v) => JsValue::from(*v),
        Value::I32(v) => JsValue::from(*v),
        Value::U8(v) => JsValue::from(*v),
        Value::U16(v) => JsValue::from(*v),
        Value::U32(v) => JsValue::from(*v),
        Value::I64(v) if v.unsigned_abs() <= MAX_SAFE_INTEGER => JsValue::from_f64(*v as f64),
        Value::U64(v) if *v <= MAX_SAFE_INTEGER => JsValue::from_f64(*v as f64),
        Value::I64(_) | Value::U64(_) => {
            return Err(Error::unsupported_feature(format!(
                "D1 binds integers as JS numbers, which cannot hold {value:?} exactly"
            )));
        }
        Value::F32(v) => JsValue::from(*v),
        Value::F64(v) => JsValue::from(*v),
        Value::String(v) => JsValue::from_str(v),
        Value::Bytes(v) => Uint8Array::from(&v[..]).into(),
        // A `Vec<scalar>` / document collection (`List`) or a bare
        // `#[document]` embed (`Object`) is stored as JSON text.
        Value::List(_) | Value::Object(_) => JsValue::from_str(
            &toasty_sql::json::to_string(value).expect("serialize document value to JSON"),
        ),
        _ => {
            return Err(Error::unsupported_feature(format!(
                "D1 cannot bind {value:?}"
            )));
        }
    })
}

/// Converts a JS value returned by D1 to a Toasty value of type `ty`.
pub(crate) fn from_js(value: JsValue, ty: &stmt::Type) -> Result<Value> {
    if value.is_null() || value.is_undefined() {
        return Ok(Value::Null);
    }

    if let Some(v) = value.as_f64() {
        // D1 rounds a 64-bit integer to the nearest JS number, so a value
        // past the safe range may not be the stored one.
        if matches!(ty, stmt::Type::I64 | stmt::Type::U64) && v.abs() > MAX_SAFE_INTEGER as f64 {
            return Err(Error::unsupported_feature(format!(
                "D1 returned the integer {v} as a JS number, which cannot hold it exactly"
            )));
        }

        return Ok(match ty {
            stmt::Type::Bool => Value::Bool(v != 0.0),
            stmt::Type::I8 => Value::I8(v as i8),
            stmt::Type::I16 => Value::I16(v as i16),
            stmt::Type::I32 => Value::I32(v as i32),
            stmt::Type::I64 => Value::I64(v as i64),
            stmt::Type::U8 => Value::U8(v as u8),
            stmt::Type::U16 => Value::U16(v as u16),
            stmt::Type::U32 => Value::U32(v as u32),
            stmt::Type::U64 => Value::U64(v as u64),
            stmt::Type::F32 => Value::F32(v as f32),
            stmt::Type::F64 => Value::F64(v),
            _ => todo!("number for ty={ty:#?}"),
        });
    }

    if let Some(v) = value.as_string() {
        return Ok(match ty {
            stmt::Type::Uuid => Value::Uuid(v.parse().expect("text is a valid uuid")),
            stmt::Type::List(elem) => toasty_sql::json::list_from_str(&v, elem)
                .expect("D1 returned non-JSON for a collection column"),
            // A bare `#[document]` column (`Type::Object`) decodes
            // shape-directed to the named `Value::Object` wire form; the
            // engine raises it to the embed's positional record.
            stmt::Type::Object => toasty_sql::json::from_str(&v, ty)
                .expect("D1 returned non-JSON for a document column"),
            _ => Value::String(v),
        });
    }

    Ok(match ty {
        // D1 returns a BLOB as an array of byte values.
        stmt::Type::Bytes => Value::Bytes(Uint8Array::new(&value).to_vec()),
        _ => todo!("value={value:?} ty={ty:#?}"),
    })
}

/// Converts a JS value returned by D1 to a Toasty value without a known
/// type. JS numbers do not keep SQLite's storage class, so a whole number
/// becomes `I64` and any other number `F64`.
pub(crate) fn from_js_infer(value: JsValue) -> Value {
    if value.is_null() || value.is_undefined() {
        Value::Null
    } else if let Some(v) = value.as_f64() {
        if v.fract() == 0.0 {
            Value::I64(v as i64)
        } else {
            Value::F64(v)
        }
    } else if let Some(v) = value.as_string() {
        Value::String(v)
    } else {
        Value::Bytes(Uint8Array::new(&value).to_vec())
    }
}
