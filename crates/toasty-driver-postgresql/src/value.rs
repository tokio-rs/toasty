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
            // List → bind as a PostgreSQL array. The prepared statement
            // already declared the param as an array type (see
            // `db::Type::List` → `to_postgres_type`); the element PG type
            // drives the per-item conversion.
            (stmt::Value::List(items), _) => {
                let Kind::Array(elem) = ty.kind() else {
                    return Err(format!("Value::List bound to non-array PG type {ty:?}").into());
                };
                list_to_sql(items, elem, ty, out)
            }
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

/// Convert a `Value::List` of scalars into the PG array wire format. NULLs in
/// the list become SQL NULLs in the array; PostgreSQL arrays carry NULLs
/// natively.
fn list_to_sql(
    items: &[CoreValue],
    elem: &Type,
    array_ty: &Type,
    out: &mut BytesMut,
) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
    /// Build a `Vec<Option<T>>` from `items`, mapping each entry through `f`.
    /// Used to share the array-binding boilerplate across element types.
    fn collect<T, F>(
        items: &[CoreValue],
        mut f: F,
    ) -> std::result::Result<Vec<Option<T>>, Box<dyn std::error::Error + Sync + Send>>
    where
        F: FnMut(&CoreValue) -> std::result::Result<T, Box<dyn std::error::Error + Sync + Send>>,
    {
        items
            .iter()
            .map(|v| match v {
                CoreValue::Null => Ok(None),
                other => f(other).map(Some),
            })
            .collect()
    }

    match *elem {
        Type::BOOL => {
            let v = collect(items, |x| match x {
                CoreValue::Bool(b) => Ok(*b),
                _ => Err(format!("expected Bool in BOOL[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        Type::INT2 => {
            let v = collect(
                items,
                |x| -> std::result::Result<i16, Box<dyn std::error::Error + Sync + Send>> {
                    match x {
                        CoreValue::I8(n) => Ok(*n as i16),
                        CoreValue::I16(n) => Ok(*n),
                        CoreValue::U8(n) => Ok(*n as i16),
                        CoreValue::U16(n) => i16::try_from(*n)
                            .map_err(|_| format!("u16 {n} out of i16 range").into()),
                        _ => Err(format!("expected small int in INT2[] array, got {x:?}").into()),
                    }
                },
            )?;
            v.to_sql(array_ty, out)
        }
        Type::INT4 => {
            let v = collect(
                items,
                |x| -> std::result::Result<i32, Box<dyn std::error::Error + Sync + Send>> {
                    match x {
                        CoreValue::I8(n) => Ok(*n as i32),
                        CoreValue::I16(n) => Ok(*n as i32),
                        CoreValue::I32(n) => Ok(*n),
                        CoreValue::U8(n) => Ok(*n as i32),
                        CoreValue::U16(n) => Ok(*n as i32),
                        CoreValue::U32(n) => i32::try_from(*n)
                            .map_err(|_| format!("u32 {n} out of i32 range").into()),
                        _ => Err(format!("expected int in INT4[] array, got {x:?}").into()),
                    }
                },
            )?;
            v.to_sql(array_ty, out)
        }
        Type::INT8 => {
            let v = collect(
                items,
                |x| -> std::result::Result<i64, Box<dyn std::error::Error + Sync + Send>> {
                    match x {
                        CoreValue::I8(n) => Ok(*n as i64),
                        CoreValue::I16(n) => Ok(*n as i64),
                        CoreValue::I32(n) => Ok(*n as i64),
                        CoreValue::I64(n) => Ok(*n),
                        CoreValue::U8(n) => Ok(*n as i64),
                        CoreValue::U16(n) => Ok(*n as i64),
                        CoreValue::U32(n) => Ok(*n as i64),
                        CoreValue::U64(n) => i64::try_from(*n)
                            .map_err(|_| format!("u64 {n} out of i64 range").into()),
                        _ => Err(format!("expected int in INT8[] array, got {x:?}").into()),
                    }
                },
            )?;
            v.to_sql(array_ty, out)
        }
        Type::FLOAT4 => {
            let v = collect(items, |x| match x {
                CoreValue::F32(n) => Ok(*n),
                CoreValue::F64(n) => Ok(*n as f32),
                _ => Err(format!("expected float in FLOAT4[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        Type::FLOAT8 => {
            let v = collect(items, |x| match x {
                CoreValue::F32(n) => Ok(*n as f64),
                CoreValue::F64(n) => Ok(*n),
                _ => Err(format!("expected float in FLOAT8[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        Type::TEXT | Type::VARCHAR => {
            let v = collect(items, |x| match x {
                CoreValue::String(s) => Ok(s.clone()),
                _ => Err(format!("expected String in TEXT[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        Type::BYTEA => {
            let v = collect(items, |x| match x {
                CoreValue::Bytes(b) => Ok(b.clone()),
                _ => Err(format!("expected Bytes in BYTEA[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        Type::UUID => {
            let v = collect(items, |x| match x {
                CoreValue::Uuid(u) => Ok(*u),
                _ => Err(format!("expected Uuid in UUID[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        #[cfg(feature = "rust_decimal")]
        Type::NUMERIC => {
            let v = collect(items, |x| match x {
                CoreValue::Decimal(d) => Ok(*d),
                _ => Err(format!("expected Decimal in NUMERIC[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        #[cfg(feature = "jiff")]
        Type::TIMESTAMPTZ => {
            let v = collect(items, |x| match x {
                CoreValue::Timestamp(t) => Ok(*t),
                _ => Err(format!("expected Timestamp in TIMESTAMPTZ[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        #[cfg(feature = "jiff")]
        Type::DATE => {
            let v = collect(items, |x| match x {
                CoreValue::Date(d) => Ok(*d),
                _ => Err(format!("expected Date in DATE[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        #[cfg(feature = "jiff")]
        Type::TIME => {
            let v = collect(items, |x| match x {
                CoreValue::Time(t) => Ok(*t),
                _ => Err(format!("expected Time in TIME[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        #[cfg(feature = "jiff")]
        Type::TIMESTAMP => {
            let v = collect(items, |x| match x {
                CoreValue::DateTime(d) => Ok(*d),
                _ => Err(format!("expected DateTime in TIMESTAMP[] array, got {x:?}").into()),
            })?;
            v.to_sql(array_ty, out)
        }
        _ => Err(format!("unsupported PG array element type: {elem:?}").into()),
    }
}
