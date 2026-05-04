use toasty_core::stmt::{self, Value as CoreValue};
use tokio_postgres::{
    Column, Row,
    types::{IsNull, Kind, ToSql, Type, private::BytesMut, to_sql_checked},
};

/// Wrapper for reading string values from PostgreSQL enum columns.
///
/// The standard `String::FromSql::accepts()` rejects custom enum types.
/// This wrapper accepts `Kind::Enum` types and reads the value as UTF-8 text.
struct EnumString(String);

impl<'a> postgres_types::FromSql<'a> for EnumString {
    fn from_sql(
        _ty: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(EnumString(
            std::str::from_utf8(raw)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Sync + Send>)?
                .to_string(),
        ))
    }

    fn accepts(ty: &Type) -> bool {
        matches!(ty.kind(), Kind::Enum(_))
    }
}

#[derive(Debug)]
pub struct Value(pub(crate) CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this PostgreSQL driver value into the core Toasty value.
    pub fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a PostgreSQL value within a row to a Toasty value.
    pub fn from_sql(index: usize, row: &Row, column: &Column, expected_ty: &stmt::Type) -> Self {
        // Gets the value from the row as Option<T> and return stmt::Value::Null if the Option is
        // None.
        macro_rules! get_or_return_null {
            ($ty:ty) => {{
                match row.get::<usize, Option<$ty>>(index) {
                    Some(inner) => inner,
                    None => return Self(stmt::Value::Null),
                }
            }};
        }

        // NOTE: unfortunately, the inner representation of the PostgreSQL type enum is not
        // accessible, so we must manually match each type like so.
        let core_value = if column.type_() == &Type::TEXT || column.type_() == &Type::VARCHAR {
            let v = get_or_return_null!(String);
            match expected_ty {
                stmt::Type::String => stmt::Value::String(v),
                stmt::Type::Uuid => stmt::Value::Uuid(
                    v.parse()
                        .unwrap_or_else(|_| panic!("uuid could not be parsed from text")),
                ),
                _ => stmt::Value::String(v), // Default to string
            }
        } else if column.type_() == &Type::BOOL {
            stmt::Value::Bool(get_or_return_null!(bool))
        } else if column.type_() == &Type::INT2 {
            let v = get_or_return_null!(i16);
            match expected_ty {
                stmt::Type::I8 => stmt::Value::I8(v as i8),
                stmt::Type::I16 => stmt::Value::I16(v),
                stmt::Type::U8 => stmt::Value::U8(
                    u8::try_from(v).unwrap_or_else(|_| panic!("u8 value out of range: {v}")),
                ),
                stmt::Type::U16 => stmt::Value::U16(v as u16),
                _ => panic!("unexpected type for INT2: {expected_ty:#?}"),
            }
        } else if column.type_() == &Type::INT4 {
            let v = get_or_return_null!(i32);
            match expected_ty {
                stmt::Type::I32 => stmt::Value::I32(v),
                stmt::Type::U16 => stmt::Value::U16(
                    u16::try_from(v).unwrap_or_else(|_| panic!("u16 value out of range: {v}")),
                ),
                stmt::Type::U32 => stmt::Value::U32(v as u32),
                _ => stmt::Value::I32(v), // Default fallback
            }
        } else if column.type_() == &Type::INT8 {
            let v = get_or_return_null!(i64);
            match expected_ty {
                stmt::Type::I64 => stmt::Value::I64(v),
                stmt::Type::U32 => stmt::Value::U32(
                    u32::try_from(v).unwrap_or_else(|_| panic!("u32 value out of range: {v}")),
                ),
                stmt::Type::U64 => stmt::Value::U64(
                    u64::try_from(v).unwrap_or_else(|_| panic!("u64 value out of range: {v}")),
                ),
                _ => stmt::Value::I64(v), // Default fallback
            }
        } else if column.type_() == &Type::UUID {
            let v = get_or_return_null!(uuid::Uuid);
            match expected_ty {
                stmt::Type::Uuid => stmt::Value::Uuid(v),
                stmt::Type::String => stmt::Value::String(v.to_string()),
                _ => stmt::Value::Uuid(v),
            }
        } else if column.type_() == &Type::BYTEA {
            let v = get_or_return_null!(Vec<u8>);
            match expected_ty {
                stmt::Type::Uuid => stmt::Value::Uuid(v.try_into().expect("invalid uuid bytes")),
                stmt::Type::Bytes => stmt::Value::Bytes(v),
                _ => todo!(
                    "unsupported conversion from {:#?} to {expected_ty:?}",
                    column.type_()
                ),
            }
        } else if column.type_() == &Type::TIMESTAMPTZ {
            #[cfg(feature = "jiff")]
            {
                stmt::Value::Timestamp(get_or_return_null!(jiff::Timestamp))
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIMESTAMPTZ requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::TIMESTAMP {
            #[cfg(feature = "jiff")]
            {
                stmt::Value::DateTime(get_or_return_null!(jiff::civil::DateTime))
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIMESTAMP requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::DATE {
            #[cfg(feature = "jiff")]
            {
                stmt::Value::Date(get_or_return_null!(jiff::civil::Date))
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("DATE requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::TIME {
            #[cfg(feature = "jiff")]
            {
                stmt::Value::Time(get_or_return_null!(jiff::civil::Time))
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIME requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::FLOAT4 {
            let v = get_or_return_null!(f32);
            match expected_ty {
                stmt::Type::F32 => stmt::Value::F32(v),
                stmt::Type::F64 => stmt::Value::F64(v as f64),
                _ => panic!("unexpected type for FLOAT4: {expected_ty:#?}"),
            }
        } else if column.type_() == &Type::FLOAT8 {
            let v = get_or_return_null!(f64);
            match expected_ty {
                stmt::Type::F32 => stmt::Value::F32(v as f32),
                stmt::Type::F64 => stmt::Value::F64(v),
                _ => panic!("unexpected type for FLOAT8: {expected_ty:#?}"),
            }
        } else if column.type_() == &Type::NUMERIC {
            #[cfg(feature = "rust_decimal")]
            {
                stmt::Value::Decimal(get_or_return_null!(rust_decimal::Decimal))
            }
            #[cfg(not(feature = "rust_decimal"))]
            {
                panic!("NUMERIC requires rust_decimal feature to be enabled")
            }
        } else if matches!(column.type_().kind(), Kind::Array(_)) {
            return Self(read_array(index, row, column.type_(), expected_ty));
        } else if matches!(column.type_().kind(), Kind::Enum(_)) {
            // Native database enum types (CREATE TYPE ... AS ENUM) are read as strings.
            // We use EnumString instead of String because String::FromSql::accepts()
            // rejects custom enum types.
            match row.get::<usize, Option<EnumString>>(index) {
                Some(EnumString(v)) => stmt::Value::String(v),
                None => return Self(stmt::Value::Null),
            }
        } else {
            todo!(
                "implement PostgreSQL to toasty conversion for `{:#?}`",
                column.type_()
            );
        };

        Value(core_value)
    }
}

impl ToSql for Value {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        match (&self.0, ty) {
            (stmt::Value::Bool(value), _) => value.to_sql(ty, out),
            (stmt::Value::I8(value), &Type::INT2) => (*value as i16).to_sql(ty, out),
            (stmt::Value::I8(value), &Type::INT4) => (*value as i32).to_sql(ty, out),
            (stmt::Value::I8(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::I16(value), &Type::INT2) => value.to_sql(ty, out),
            (stmt::Value::I16(value), &Type::INT4) => (*value as i32).to_sql(ty, out),
            (stmt::Value::I16(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::I32(value), &Type::INT4) => value.to_sql(ty, out),
            (stmt::Value::I32(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::I64(value), &Type::INT4) => (*value as i32).to_sql(ty, out),
            (stmt::Value::I64(value), &Type::INT8) => value.to_sql(ty, out),
            (stmt::Value::U8(value), &Type::INT2) => (*value as i16).to_sql(ty, out),
            (stmt::Value::U8(value), &Type::INT4) => (*value as i32).to_sql(ty, out),
            (stmt::Value::U8(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::U16(value), &Type::INT4) => (*value as i32).to_sql(ty, out),
            (stmt::Value::U16(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::U32(value), &Type::INT8) => (*value as i64).to_sql(ty, out),
            (stmt::Value::U64(value), &Type::INT8) => {
                if *value > i64::MAX as u64 {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "u64 value {} exceeds i64::MAX ({}), cannot store in PostgreSQL BIGINT",
                            value,
                            i64::MAX
                        ),
                    )));
                }
                (*value as i64).to_sql(ty, out)
            }
            (stmt::Value::F32(value), &Type::FLOAT4) => value.to_sql(ty, out),
            (stmt::Value::F32(value), &Type::FLOAT8) => (*value as f64).to_sql(ty, out),
            (stmt::Value::F64(value), &Type::FLOAT4) => (*value as f32).to_sql(ty, out),
            (stmt::Value::F64(value), &Type::FLOAT8) => value.to_sql(ty, out),
            (stmt::Value::Null, _) => Ok(IsNull::Yes),
            (stmt::Value::String(value), _) => value.to_sql(ty, out),
            (stmt::Value::Bytes(value), &Type::BYTEA) => value.to_sql(ty, out),
            (stmt::Value::Uuid(value), &Type::UUID) => value.to_sql(ty, out),
            #[cfg(feature = "rust_decimal")]
            (stmt::Value::Decimal(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::Timestamp(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::Date(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::Time(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::DateTime(value), _) => value.to_sql(ty, out),
            (stmt::Value::List(items), _) => list_to_sql(items, ty, out),
            (value, _) => todo!("unsupported Value for PostgreSQL type: {value:#?}, type: {ty:#?}"),
        }
    }

    fn accepts(ty: &Type) -> bool {
        matches!(
            *ty,
            Type::BOOL
                | Type::INT2
                | Type::INT4
                | Type::INT8
                | Type::TEXT
                | Type::FLOAT4
                | Type::FLOAT8
                | Type::VARCHAR
                | Type::BYTEA
                | Type::UUID
                | Type::NUMERIC
                | Type::TIMESTAMP
                | Type::TIMESTAMPTZ
                | Type::DATE
                | Type::TIME
        ) || matches!(ty.kind(), Kind::Enum(_) | Kind::Array(_))
    }
    to_sql_checked!();
}

/// Read a PostgreSQL array column into a `Value::List`. Dispatches on the
/// column's element type and the expected `stmt::Type::List(elem)`.
fn read_array(index: usize, row: &Row, ty: &Type, expected_ty: &stmt::Type) -> stmt::Value {
    let stmt::Type::List(elem_ty) = expected_ty else {
        panic!("array column expects stmt::Type::List, got {expected_ty:?}");
    };
    let Kind::Array(elem_pg) = ty.kind() else {
        panic!("read_array called on non-array PG type: {ty:?}");
    };

    macro_rules! read_and_map {
        ($t:ty, $wrap:expr) => {{
            let Some(v) = row.get::<usize, Option<Vec<$t>>>(index) else {
                return stmt::Value::Null;
            };
            stmt::Value::List(v.into_iter().map($wrap).collect())
        }};
    }

    match (&**elem_ty, elem_pg) {
        (stmt::Type::Bool, _) => read_and_map!(bool, stmt::Value::Bool),
        (stmt::Type::I8, _) => read_and_map!(i16, |v| stmt::Value::I8(v as i8)),
        (stmt::Type::I16, _) => read_and_map!(i16, stmt::Value::I16),
        (stmt::Type::I32, _) => read_and_map!(i32, stmt::Value::I32),
        (stmt::Type::I64, _) => read_and_map!(i64, stmt::Value::I64),
        (stmt::Type::U8, _) => read_and_map!(i16, |v| stmt::Value::U8(v as u8)),
        (stmt::Type::U16, _) => read_and_map!(i32, |v| stmt::Value::U16(v as u16)),
        (stmt::Type::U32, _) => read_and_map!(i64, |v| stmt::Value::U32(v as u32)),
        (stmt::Type::U64, _) => read_and_map!(i64, |v| stmt::Value::U64(v as u64)),
        (stmt::Type::F32, _) => read_and_map!(f32, stmt::Value::F32),
        (stmt::Type::F64, _) => read_and_map!(f64, stmt::Value::F64),
        (stmt::Type::String, _) => read_and_map!(String, stmt::Value::String),
        (stmt::Type::Uuid, _) => read_and_map!(uuid::Uuid, stmt::Value::Uuid),
        (elem, _) => todo!("read_array for element type {elem:?}"),
    }
}

/// Bind a `Value::List` to a PostgreSQL array column. The element type is
/// taken from the column's `Type` (`Kind::Array(elem)`); each list element
/// is converted to the matching Rust type and the whole `Vec` is bound via
/// the standard `postgres-types` array encoder.
fn list_to_sql(
    items: &[stmt::Value],
    ty: &Type,
    out: &mut BytesMut,
) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
    let Kind::Array(elem) = ty.kind() else {
        return Err(format!("Value::List can only bind to an array type, got {ty:?}").into());
    };

    macro_rules! collect_and_bind {
        ($t:ty, $extract:expr) => {{
            let vec: Vec<$t> = items.iter().map($extract).collect::<Result<_, _>>()?;
            return vec.to_sql(ty, out);
        }};
    }

    match *elem {
        Type::BOOL => collect_and_bind!(bool, |v| match v {
            stmt::Value::Bool(b) => Ok(*b),
            other => Err(format!("expected Bool, got {other:?}")),
        }),
        Type::INT2 => collect_and_bind!(i16, |v| match v {
            stmt::Value::I8(b) => Ok(*b as i16),
            stmt::Value::I16(b) => Ok(*b),
            stmt::Value::U8(b) => Ok(*b as i16),
            other => Err(format!("expected i16-compatible, got {other:?}")),
        }),
        Type::INT4 => collect_and_bind!(i32, |v| match v {
            stmt::Value::I32(b) => Ok(*b),
            stmt::Value::U16(b) => Ok(*b as i32),
            other => Err(format!("expected i32-compatible, got {other:?}")),
        }),
        Type::INT8 => collect_and_bind!(i64, |v| match v {
            stmt::Value::I64(b) => Ok(*b),
            stmt::Value::U32(b) => Ok(*b as i64),
            stmt::Value::U64(b) if *b <= i64::MAX as u64 => Ok(*b as i64),
            other => Err(format!("expected i64-compatible, got {other:?}")),
        }),
        Type::FLOAT4 => collect_and_bind!(f32, |v| match v {
            stmt::Value::F32(b) => Ok(*b),
            other => Err(format!("expected F32, got {other:?}")),
        }),
        Type::FLOAT8 => collect_and_bind!(f64, |v| match v {
            stmt::Value::F64(b) => Ok(*b),
            other => Err(format!("expected F64, got {other:?}")),
        }),
        Type::TEXT | Type::VARCHAR => collect_and_bind!(String, |v| match v {
            stmt::Value::String(s) => Ok(s.clone()),
            other => Err(format!("expected String, got {other:?}")),
        }),
        Type::UUID => collect_and_bind!(uuid::Uuid, |v| match v {
            stmt::Value::Uuid(u) => Ok(*u),
            other => Err(format!("expected Uuid, got {other:?}")),
        }),
        _ => Err(format!("unsupported array element type {elem:?}").into()),
    }
}
