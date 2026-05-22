//! Value-conversion between Toasty's `stmt::Value` and `turso::Value`.
//!
//! Scope is intentionally limited to SQLite's classic storage classes
//! (integer, real, text, blob, null) plus the `Vec<scalar>` JSON
//! round-trip the SQLite driver already does. Turso's richer typing —
//! `experimental_custom_types`, decimal, native date/time — is still
//! moving upstream, so we leave those cases as `todo!()` for now and
//! revisit once the upstream surface settles. The current shape lets
//! Turso ride on the SQLite-flavored serializer without an
//! independent type system.

use toasty_core::stmt::{self, Value as CoreValue};
use turso::Value as TursoValue;

/// Converts a [`toasty_core::stmt::Value`] to a [`turso::Value`].
pub(crate) fn to_turso(value: &CoreValue) -> TursoValue {
    match value {
        CoreValue::Bool(true) => TursoValue::Integer(1),
        CoreValue::Bool(false) => TursoValue::Integer(0),
        CoreValue::I8(v) => TursoValue::Integer(*v as i64),
        CoreValue::I16(v) => TursoValue::Integer(*v as i64),
        CoreValue::I32(v) => TursoValue::Integer(*v as i64),
        CoreValue::I64(v) => TursoValue::Integer(*v),
        CoreValue::U8(v) => TursoValue::Integer(*v as i64),
        CoreValue::U16(v) => TursoValue::Integer(*v as i64),
        CoreValue::U32(v) => TursoValue::Integer(*v as i64),
        CoreValue::U64(v) => TursoValue::Integer(*v as i64),
        CoreValue::F32(v) => TursoValue::Real(*v as f64),
        CoreValue::F64(v) => TursoValue::Real(*v),
        CoreValue::String(v) => TursoValue::Text(v.clone()),
        CoreValue::Bytes(v) => TursoValue::Blob(v.clone()),
        CoreValue::Null => TursoValue::Null,
        CoreValue::List(_) => TursoValue::Text(value_list_to_json_text(value)),
        _ => todo!("to_turso: value = {value:#?}"),
    }
}

/// Converts a [`turso::Value`] back to a [`toasty_core::stmt::Value`] using
/// the expected return type to interpret the raw SQLite value correctly.
pub(crate) fn from_turso(value: TursoValue, ty: &stmt::Type) -> CoreValue {
    match value {
        TursoValue::Null => CoreValue::Null,
        TursoValue::Integer(v) => match ty {
            stmt::Type::Bool => CoreValue::Bool(v != 0),
            stmt::Type::I8 => CoreValue::I8(v as i8),
            stmt::Type::I16 => CoreValue::I16(v as i16),
            stmt::Type::I32 => CoreValue::I32(v as i32),
            stmt::Type::I64 => CoreValue::I64(v),
            stmt::Type::U8 => CoreValue::U8(v as u8),
            stmt::Type::U16 => CoreValue::U16(v as u16),
            stmt::Type::U32 => CoreValue::U32(v as u32),
            stmt::Type::U64 => CoreValue::U64(v as u64),
            _ => todo!("from_turso integer: ty={ty:#?}"),
        },
        TursoValue::Real(v) => match ty {
            stmt::Type::F32 => CoreValue::F32(v as f32),
            stmt::Type::F64 => CoreValue::F64(v),
            _ => todo!("from_turso real: ty={ty:#?}"),
        },
        TursoValue::Text(v) => match ty {
            stmt::Type::Uuid => CoreValue::Uuid(v.parse().expect("text is a valid uuid")),
            stmt::Type::List(elem) => json_text_to_value_list(&v, elem),
            _ => CoreValue::String(v),
        },
        TursoValue::Blob(v) => match ty {
            stmt::Type::Bytes => CoreValue::Bytes(v),
            _ => todo!("from_turso blob: value={v:#?}"),
        },
    }
}

fn value_list_to_json_text(value: &CoreValue) -> String {
    let json = toasty_sql::value_json::value_list_to_json(value);
    serde_json::to_string(&json).expect("serialize Vec<scalar> to JSON")
}

fn json_text_to_value_list(text: &str, elem_ty: &stmt::Type) -> CoreValue {
    let json: serde_json::Value =
        serde_json::from_str(text).expect("Turso returned non-JSON for a Vec<scalar> column");
    toasty_sql::value_json::value_list_from_json(json, elem_ty)
}
