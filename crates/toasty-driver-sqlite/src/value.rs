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
                stmt::Type::List(elem) => json_text_to_value_list(&value, elem),
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
            Value::List(_) => Ok(ToSqlOutput::Owned(SqlValue::Text(value_list_to_json_text(
                &self.0,
            )))),
            _ => todo!("value = {:#?}", self.0),
        }
    }
}

/// Encode a `Value::List` as a JSON document for a SQLite TEXT column
/// (consumed by the JSON1 extension via `json_each` and friends).
fn value_list_to_json_text(value: &CoreValue) -> String {
    let CoreValue::List(items) = value else {
        unreachable!("value_list_to_json_text called on {value:?}")
    };
    let items: Vec<serde_json::Value> = items.iter().map(scalar_to_json).collect();
    serde_json::to_string(&serde_json::Value::Array(items))
        .expect("serializing scalar list to JSON")
}

/// Decode a JSON document stored in a SQLite TEXT column into a
/// `Value::List`. The element type comes from the schema.
fn json_text_to_value_list(text: &str, elem_ty: &stmt::Type) -> CoreValue {
    let json: serde_json::Value =
        serde_json::from_str(text).expect("SQLite returned non-JSON for a Vec<scalar> column");
    let serde_json::Value::Array(items) = json else {
        panic!("expected JSON array for Vec<scalar> column, got {json:?}")
    };
    let items = items
        .into_iter()
        .map(|v| json_to_scalar(v, elem_ty))
        .collect();
    stmt::Value::List(items)
}

fn scalar_to_json(value: &CoreValue) -> serde_json::Value {
    use serde_json::Value as J;
    match value {
        CoreValue::Null => J::Null,
        CoreValue::Bool(v) => J::Bool(*v),
        CoreValue::I8(v) => J::Number((*v).into()),
        CoreValue::I16(v) => J::Number((*v).into()),
        CoreValue::I32(v) => J::Number((*v).into()),
        CoreValue::I64(v) => J::Number((*v).into()),
        CoreValue::U8(v) => J::Number((*v).into()),
        CoreValue::U16(v) => J::Number((*v).into()),
        CoreValue::U32(v) => J::Number((*v).into()),
        CoreValue::U64(v) => J::Number((*v).into()),
        CoreValue::F32(v) => serde_json::Number::from_f64((*v).into())
            .map(J::Number)
            .unwrap_or(J::Null),
        CoreValue::F64(v) => serde_json::Number::from_f64(*v)
            .map(J::Number)
            .unwrap_or(J::Null),
        CoreValue::String(v) => J::String(v.clone()),
        CoreValue::Uuid(v) => J::String(v.to_string()),
        _ => todo!("encode {value:?} as JSON for a Vec<scalar> field"),
    }
}

fn json_to_scalar(json: serde_json::Value, ty: &stmt::Type) -> CoreValue {
    use serde_json::Value as J;
    match (ty, json) {
        (_, J::Null) => CoreValue::Null,
        (stmt::Type::Bool, J::Bool(v)) => CoreValue::Bool(v),
        (stmt::Type::String, J::String(v)) => CoreValue::String(v),
        (stmt::Type::Uuid, J::String(v)) => {
            CoreValue::Uuid(v.parse().expect("invalid UUID in JSON"))
        }
        (stmt::Type::I8, J::Number(n)) => CoreValue::I8(n.as_i64().unwrap() as i8),
        (stmt::Type::I16, J::Number(n)) => CoreValue::I16(n.as_i64().unwrap() as i16),
        (stmt::Type::I32, J::Number(n)) => CoreValue::I32(n.as_i64().unwrap() as i32),
        (stmt::Type::I64, J::Number(n)) => CoreValue::I64(n.as_i64().unwrap()),
        (stmt::Type::U8, J::Number(n)) => CoreValue::U8(n.as_u64().unwrap() as u8),
        (stmt::Type::U16, J::Number(n)) => CoreValue::U16(n.as_u64().unwrap() as u16),
        (stmt::Type::U32, J::Number(n)) => CoreValue::U32(n.as_u64().unwrap() as u32),
        (stmt::Type::U64, J::Number(n)) => CoreValue::U64(n.as_u64().unwrap()),
        (stmt::Type::F32, J::Number(n)) => CoreValue::F32(n.as_f64().unwrap() as f32),
        (stmt::Type::F64, J::Number(n)) => CoreValue::F64(n.as_f64().unwrap()),
        (ty, json) => todo!("decode JSON value {json:?} as {ty:?}"),
    }
}
