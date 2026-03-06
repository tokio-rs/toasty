use rusqlite::{
    types::{ToSql, ToSqlOutput, Value as SqlValue, ValueRef},
    Row,
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
            Some(SqlValue::Text(value)) => match ty {
                stmt::Type::Uuid => stmt::Value::Uuid(value.parse().expect("text is a valid uuid")),
                _ => stmt::Value::String(value),
            },
            Some(SqlValue::Blob(value)) => match ty {
                stmt::Type::Bytes => stmt::Value::Bytes(value),
                _ => todo!("value={value:#?}"),
            },
            None => stmt::Value::Null,
            _ => todo!("value={value:#?}"),
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
            Value::String(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Text(v.as_bytes()))),
            Value::Bytes(v) => Ok(ToSqlOutput::Borrowed(ValueRef::Blob(&v[..]))),
            Value::Null => Ok(ToSqlOutput::Owned(SqlValue::Null)),
            _ => todo!("value = {:#?}", self.0),
        }
    }
}
