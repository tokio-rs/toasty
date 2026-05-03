use rusqlite::{
    Row,
    types::{ToSql, ToSqlOutput, Value as SqlValue, ValueRef},
};
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this SQLite driver value into the core Toasty value.
    pub fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a SQLite value within a row to a Toasty value.
    pub fn from_sql(row: &Row, index: usize, ty: &stmt::Type) -> Self {
        let value: Option<SqlValue> = row.get(index).unwrap();

        let core_value = match value {
            Some(SqlValue::Null) => stmt::Value::Null,
            Some(SqlValue::Integer(value)) => match ty {
                stmt::Type::Bool => stmt::Value::Bool(value != 0),
                stmt::Type::I8 => stmt::Value::I8(value as i8),
                stmt::Type::I16 => stmt::Value::I16(value as i16),
                stmt::Type::I32 => stmt::Value::I32(value as i32),
                stmt::Type::I64 => stmt::Value::I64(value),
                stmt::Type::U8 => stmt::Value::U8(value as u8),
                stmt::Type::U16 => stmt::Value::U16(value as u16),
                stmt::Type::U32 => stmt::Value::U32(value as u32),
                stmt::Type::U64 => stmt::Value::U64(value as u64),
                _ => todo!("ty={ty:#?}"),
            },
            Some(SqlValue::Real(value)) => match ty {
                stmt::Type::F32 => stmt::Value::F32(value as f32),
                stmt::Type::F64 => stmt::Value::F64(value),
                _ => todo!("ty={ty:#?}"),
            },
            Some(SqlValue::Text(value)) => match ty {
                stmt::Type::Uuid => stmt::Value::Uuid(value.parse().expect("text is a valid uuid")),
                stmt::Type::List(elem) => decode_json_list(&value, elem),
                _ => stmt::Value::String(value),
            },
            Some(SqlValue::Blob(value)) => match ty {
                stmt::Type::Bytes => stmt::Value::Bytes(value),
                _ => todo!("value={value:#?}"),
            },
            None => stmt::Value::Null,
        };

        Value(core_value)
    }
}

/// Decode a SQLite text payload (a JSON array) into `Value::List` whose
/// elements have type `elem`.
fn decode_json_list(text: &str, elem: &stmt::Type) -> stmt::Value {
    let json: serde_json::Value = serde_json::from_str(text).expect("invalid JSON in list column");
    let serde_json::Value::Array(items) = json else {
        panic!("expected JSON array; got {json:#?}");
    };
    stmt::Value::List(
        items
            .into_iter()
            .map(|item| json_to_value(item, elem))
            .collect(),
    )
}

fn json_to_value(json: serde_json::Value, ty: &stmt::Type) -> stmt::Value {
    match (json, ty) {
        (serde_json::Value::Null, _) => stmt::Value::Null,
        (serde_json::Value::String(s), stmt::Type::String) => stmt::Value::String(s),
        (serde_json::Value::String(s), stmt::Type::Uuid) => {
            stmt::Value::Uuid(s.parse().expect("invalid uuid in JSON list element"))
        }
        (serde_json::Value::Bool(b), stmt::Type::Bool) => stmt::Value::Bool(b),
        (serde_json::Value::Number(n), stmt::Type::I8) => {
            stmt::Value::I8(n.as_i64().expect("integer expected") as i8)
        }
        (serde_json::Value::Number(n), stmt::Type::I16) => {
            stmt::Value::I16(n.as_i64().expect("integer expected") as i16)
        }
        (serde_json::Value::Number(n), stmt::Type::I32) => {
            stmt::Value::I32(n.as_i64().expect("integer expected") as i32)
        }
        (serde_json::Value::Number(n), stmt::Type::I64) => {
            stmt::Value::I64(n.as_i64().expect("integer expected"))
        }
        (serde_json::Value::Number(n), stmt::Type::U8) => {
            stmt::Value::U8(n.as_u64().expect("unsigned integer expected") as u8)
        }
        (serde_json::Value::Number(n), stmt::Type::U16) => {
            stmt::Value::U16(n.as_u64().expect("unsigned integer expected") as u16)
        }
        (serde_json::Value::Number(n), stmt::Type::U32) => {
            stmt::Value::U32(n.as_u64().expect("unsigned integer expected") as u32)
        }
        (serde_json::Value::Number(n), stmt::Type::U64) => {
            stmt::Value::U64(n.as_u64().expect("unsigned integer expected"))
        }
        (serde_json::Value::Number(n), stmt::Type::F32) => {
            stmt::Value::F32(n.as_f64().expect("float expected") as f32)
        }
        (serde_json::Value::Number(n), stmt::Type::F64) => {
            stmt::Value::F64(n.as_f64().expect("float expected"))
        }
        (json, ty) => todo!("json={json:#?}; ty={ty:#?}"),
    }
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        use stmt::Value;

        match &self.0 {
            Value::Bool(true) => Ok(ToSqlOutput::Owned(SqlValue::Integer(1))),
            Value::Bool(false) => Ok(ToSqlOutput::Owned(SqlValue::Integer(0))),
            Value::I8(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I16(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I32(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::I64(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v))),
            Value::U8(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U16(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U32(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::U64(v) => Ok(ToSqlOutput::Owned(SqlValue::Integer(*v as i64))),
            Value::F32(v) => Ok(ToSqlOutput::Owned(SqlValue::Real(*v as f64))),
            Value::F64(v) => Ok(ToSqlOutput::Owned(SqlValue::Real(*v))),
            Value::String(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Text(v.as_bytes()))),
            Value::Bytes(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Blob(&v[..]))),
            Value::Null => Ok(ToSqlOutput::Owned(SqlValue::Null)),
            Value::List(items) => {
                // Encode as a JSON array text payload. SQLite's JSON1
                // functions operate on TEXT columns so this round-trips
                // through `decode_json_list` on read.
                let json =
                    serde_json::Value::Array(items.iter().map(value_to_json).collect::<Vec<_>>());
                Ok(ToSqlOutput::Owned(SqlValue::Text(json.to_string())))
            }
            _ => todo!("value = {:#?}", self.0),
        }
    }
}

fn value_to_json(value: &stmt::Value) -> serde_json::Value {
    match value {
        stmt::Value::Null => serde_json::Value::Null,
        stmt::Value::Bool(v) => serde_json::Value::Bool(*v),
        stmt::Value::I8(v) => serde_json::Value::Number((*v as i64).into()),
        stmt::Value::I16(v) => serde_json::Value::Number((*v as i64).into()),
        stmt::Value::I32(v) => serde_json::Value::Number((*v as i64).into()),
        stmt::Value::I64(v) => serde_json::Value::Number((*v).into()),
        stmt::Value::U8(v) => serde_json::Value::Number((*v as u64).into()),
        stmt::Value::U16(v) => serde_json::Value::Number((*v as u64).into()),
        stmt::Value::U32(v) => serde_json::Value::Number((*v as u64).into()),
        stmt::Value::U64(v) => serde_json::Value::Number((*v).into()),
        stmt::Value::F32(v) => serde_json::Number::from_f64(*v as f64)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        stmt::Value::F64(v) => serde_json::Number::from_f64(*v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        stmt::Value::String(v) => serde_json::Value::String(v.clone()),
        stmt::Value::Uuid(v) => serde_json::Value::String(v.to_string()),
        _ => todo!("value_to_json: {value:#?}"),
    }
}
