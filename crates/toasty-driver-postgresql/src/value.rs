use postgres::{
    types::{accepts, private::BytesMut, to_sql_checked, IsNull, ToSql, Type},
    Column, Row,
};
use toasty_core::stmt::{self, Value as CoreValue};

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
            match expected_ty {
                #[cfg(feature = "jiff")]
                stmt::Type::JiffTimestamp | stmt::Type::JiffZoned => {
                    get_from_row::<jiff::Timestamp>(row, index)
                }
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoDateTimeUtc => {
                    get_from_row::<chrono::DateTime<chrono::Utc>>(row, index)
                }
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoNaiveDateTime => {
                    get_from_row::<chrono::NaiveDateTime>(row, index)
                }
                _ => panic!("TIMESTAMPTZ requires the jiff or chrono feature to be enabled"),
            }
        } else if column.type_() == &Type::TIMESTAMP {
            match expected_ty {
                #[cfg(feature = "jiff")]
                stmt::Type::JiffDateTime => get_from_row::<jiff::civil::DateTime>(row, index),
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoNaiveDateTime => {
                    get_from_row::<chrono::NaiveDateTime>(row, index)
                }
                _ => panic!("TIMESTAMP requires the jiff or chrono feature to be enabled"),
            }
        } else if column.type_() == &Type::DATE {
            match expected_ty {
                #[cfg(feature = "jiff")]
                stmt::Type::JiffDate => get_from_row::<jiff::civil::Date>(row, index),
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoNaiveDate => get_from_row::<chrono::NaiveDate>(row, index),
                _ => panic!("DATE requires the jiff or chrono feature to be enabled"),
            }
        } else if column.type_() == &Type::TIME {
            match expected_ty {
                #[cfg(feature = "jiff")]
                stmt::Type::JiffTime => get_from_row::<jiff::civil::Time>(row, index),
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoNaiveTime => get_from_row::<chrono::NaiveTime>(row, index),
                _ => panic!("TIME requires the jiff or chrono feature to be enabled"),
            }
        } else if column.type_() == &Type::NUMERIC {
            #[cfg(feature = "rust_decimal")]
            {
                get_from_row::<rust_decimal::Decimal>(row, index)
            }
            #[cfg(not(feature = "rust_decimal"))]
            {
                panic!("NUMERIC requires rust_decimal feature to be enabled")
            }
        } else {
            todo!(
                "implement PostgreSQL to toasty conversion for `{:#?}`",
                column.type_()
            );
        };

        Value(core_value)
    }

    /// Returns the inferred Toasty type for this value.
    pub fn infer_ty(&self) -> stmt::Type {
        self.0.infer_ty()
    }
}

#[cfg(any(feature = "chrono", feature = "jiff", feature = "rust_decimal"))]
fn get_from_row<T>(row: &Row, index: usize) -> stmt::Value
where
    T: postgres_types::FromSqlOwned,
    T: Into<stmt::Value>,
{
    row.get::<usize, Option<T>>(index)
        .map(Into::into)
        .unwrap_or(stmt::Value::Null)
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
            (stmt::Value::Id(value), _) => value.to_string().to_sql(ty, out),
            (stmt::Value::Null, _) => Ok(IsNull::Yes),
            (stmt::Value::String(value), _) => value.to_sql(ty, out),
            (stmt::Value::Bytes(value), &Type::BYTEA) => value.to_sql(ty, out),
            (stmt::Value::Uuid(value), &Type::UUID) => value.to_sql(ty, out),
            #[cfg(feature = "rust_decimal")]
            (stmt::Value::Decimal(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::JiffTimestamp(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::JiffDate(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::JiffTime(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            (stmt::Value::JiffDateTime(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "chrono")]
            (stmt::Value::ChronoDateTimeUtc(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "chrono")]
            (stmt::Value::ChronoNaiveDateTime(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "chrono")]
            (stmt::Value::ChronoNaiveDate(value), _) => value.to_sql(ty, out),
            #[cfg(feature = "chrono")]
            (stmt::Value::ChronoNaiveTime(value), _) => value.to_sql(ty, out),
            value => todo!("{value:#?}"),
        }
    }

    accepts!(
        BOOL,
        INT2,
        INT4,
        INT8,
        TEXT,
        VARCHAR,
        BYTEA,
        UUID,
        NUMERIC,
        TIMESTAMP,
        TIMESTAMPTZ,
        DATE,
        TIME
    );
    to_sql_checked!();
}
