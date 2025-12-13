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
        // NOTE: unfortunately, the inner representation of the PostgreSQL type enum is not
        // accessible, so we must manually match each type like so.
        let core_value = if column.type_() == &Type::TEXT || column.type_() == &Type::VARCHAR {
            row.get::<usize, Option<String>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::String => stmt::Value::String(v),
                    stmt::Type::Uuid => stmt::Value::Uuid(
                        v.parse()
                            .unwrap_or_else(|_| panic!("uuid could not be parsed from text")),
                    ),
                    _ => stmt::Value::String(v), // Default to string
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::BOOL {
            row.get::<usize, Option<bool>>(index)
                .map(stmt::Value::Bool)
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::INT2 {
            row.get::<usize, Option<i16>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::I8 => stmt::Value::I8(v as i8),
                    stmt::Type::I16 => stmt::Value::I16(v),
                    stmt::Type::U8 => stmt::Value::U8(
                        u8::try_from(v).unwrap_or_else(|_| panic!("u8 value out of range: {v}")),
                    ),
                    stmt::Type::U16 => stmt::Value::U16(v as u16),
                    _ => panic!("unexpected type for INT2: {expected_ty:#?}"),
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::INT4 {
            row.get::<usize, Option<i32>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::I32 => stmt::Value::I32(v),
                    stmt::Type::U16 => stmt::Value::U16(
                        u16::try_from(v).unwrap_or_else(|_| panic!("u16 value out of range: {v}")),
                    ),
                    stmt::Type::U32 => stmt::Value::U32(v as u32),
                    _ => stmt::Value::I32(v), // Default fallback
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::INT8 {
            row.get::<usize, Option<i64>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::I64 => stmt::Value::I64(v),
                    stmt::Type::U32 => stmt::Value::U32(
                        u32::try_from(v).unwrap_or_else(|_| panic!("u32 value out of range: {v}")),
                    ),
                    stmt::Type::U64 => stmt::Value::U64(
                        u64::try_from(v).unwrap_or_else(|_| panic!("u64 value out of range: {v}")),
                    ),
                    _ => stmt::Value::I64(v), // Default fallback
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::UUID {
            row.get::<usize, Option<uuid::Uuid>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::Uuid => stmt::Value::Uuid(v),
                    stmt::Type::String => stmt::Value::String(v.to_string()),
                    _ => stmt::Value::Uuid(v),
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::BYTEA {
            row.get::<usize, Option<Vec<u8>>>(index)
                .map(|v| match expected_ty {
                    stmt::Type::Uuid => {
                        stmt::Value::Uuid(v.try_into().expect("invalid uuid bytes"))
                    }
                    stmt::Type::Bytes => stmt::Value::Bytes(v),
                    _ => todo!(
                        "unsupported conversion from {:#?} to {expected_ty:?}",
                        column.type_()
                    ),
                })
                .unwrap_or(stmt::Value::Null)
        } else if column.type_() == &Type::TIMESTAMPTZ {
            #[cfg(feature = "jiff")]
            {
                row.get::<usize, Option<jiff::Timestamp>>(index)
                    .map(stmt::Value::Timestamp)
                    .unwrap_or(stmt::Value::Null)
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIMESTAMPTZ requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::TIMESTAMP {
            #[cfg(feature = "jiff")]
            {
                row.get::<usize, Option<jiff::civil::DateTime>>(index)
                    .map(stmt::Value::DateTime)
                    .unwrap_or(stmt::Value::Null)
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIMESTAMP requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::DATE {
            #[cfg(feature = "jiff")]
            {
                row.get::<usize, Option<jiff::civil::Date>>(index)
                    .map(stmt::Value::Date)
                    .unwrap_or(stmt::Value::Null)
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("DATE requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::TIME {
            #[cfg(feature = "jiff")]
            {
                row.get::<usize, Option<jiff::civil::Time>>(index)
                    .map(stmt::Value::Time)
                    .unwrap_or(stmt::Value::Null)
            }
            #[cfg(not(feature = "jiff"))]
            {
                panic!("TIME requires jiff feature to be enabled")
            }
        } else if column.type_() == &Type::NUMERIC {
            #[cfg(feature = "rust_decimal")]
            {
                row.get::<usize, Option<rust_decimal::Decimal>>(index)
                    .map(stmt::Value::Decimal)
                    .unwrap_or(stmt::Value::Null)
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

    /// Returns the PostgreSQL type for this value.
    pub fn postgres_ty(&self) -> Type {
        match &self.0 {
            stmt::Value::Bool(_) => Type::BOOL,
            stmt::Value::I8(_) => Type::INT2,
            stmt::Value::I16(_) => Type::INT2,
            stmt::Value::I32(_) => Type::INT4,
            stmt::Value::I64(_) => Type::INT8,
            stmt::Value::U8(_) => Type::INT2,
            stmt::Value::U16(_) => Type::INT4,
            stmt::Value::U32(_) => Type::INT8,
            stmt::Value::U64(_) => Type::INT8,
            stmt::Value::Id(_) => Type::TEXT,
            stmt::Value::String(_) => Type::TEXT,
            stmt::Value::Uuid(_) => Type::UUID,
            stmt::Value::Null => Type::TEXT, // Default for NULL values
            #[cfg(feature = "rust_decimal")]
            stmt::Value::Decimal(_) => Type::NUMERIC,
            #[cfg(feature = "jiff")]
            stmt::Value::Timestamp(_) => Type::TIMESTAMPTZ,
            #[cfg(feature = "jiff")]
            stmt::Value::Date(_) => Type::DATE,
            #[cfg(feature = "jiff")]
            stmt::Value::Time(_) => Type::TIME,
            #[cfg(feature = "jiff")]
            stmt::Value::DateTime(_) => Type::TIMESTAMP,
            _ => todo!("ty: {:#?}", self.0),
        }
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
        match &self.0 {
            stmt::Value::Bool(value) => value.to_sql(ty, out),
            stmt::Value::I8(value) => match *ty {
                Type::INT2 => {
                    let value = *value as i16;
                    value.to_sql(ty, out)
                }
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::I16(value) => match *ty {
                Type::INT2 => value.to_sql(ty, out),
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::I32(value) => match *ty {
                Type::INT4 => value.to_sql(ty, out),
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            // TODO: we need to do better type management
            stmt::Value::I64(value) => match *ty {
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => value.to_sql(ty, out),
                _ => todo!("ty={ty:#?}"),
            },
            stmt::Value::U8(value) => match *ty {
                Type::INT2 => {
                    // u8 is now stored in i16 (SMALLINT)
                    let value = *value as i16;
                    value.to_sql(ty, out)
                }
                Type::INT4 => {
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!(),
            },
            stmt::Value::U16(value) => match *ty {
                Type::INT4 => {
                    // u16 is now stored in i32 (INTEGER)
                    let value = *value as i32;
                    value.to_sql(ty, out)
                }
                Type::INT8 => {
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("Unsupported PostgreSQL type for u16: {:?}", ty),
            },
            stmt::Value::U32(value) => match *ty {
                Type::INT8 => {
                    // u32 stored in BIGINT
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("Unsupported PostgreSQL type for u32: {:?}", ty),
            },
            stmt::Value::U64(value) => match *ty {
                Type::INT8 => {
                    // PostgreSQL BIGINT is signed, so validate u64 fits in i64 range
                    if *value > i64::MAX as u64 {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("u64 value {} exceeds i64::MAX ({}), cannot store in PostgreSQL BIGINT",
                                value, i64::MAX)
                        )));
                    }
                    let value = *value as i64;
                    value.to_sql(ty, out)
                }
                _ => todo!("Unsupported PostgreSQL type for u64: {:?}", ty),
            },
            stmt::Value::Id(value) => value.to_string().to_sql(ty, out),
            stmt::Value::Null => Ok(IsNull::Yes),
            stmt::Value::String(value) => value.to_sql(ty, out),
            stmt::Value::Bytes(value) => match *ty {
                Type::BYTEA => value.to_sql(ty, out),
                _ => todo!("Unsupported PostgreSQL type for bytes: {:?}", ty),
            },
            stmt::Value::Uuid(value) => match *ty {
                Type::UUID => value.to_sql(ty, out),
                Type::BYTEA => value.as_bytes().to_sql(ty, out),
                Type::TEXT => value.to_string().to_sql(ty, out),
                Type::VARCHAR => value.to_string().to_sql(ty, out),
                _ => todo!("Unsupported PostgreSQL type for UUID: {:?}", ty),
            },
            #[cfg(feature = "rust_decimal")]
            stmt::Value::Decimal(value) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            stmt::Value::Timestamp(value) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            stmt::Value::Date(value) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            stmt::Value::Time(value) => value.to_sql(ty, out),
            #[cfg(feature = "jiff")]
            stmt::Value::DateTime(value) => value.to_sql(ty, out),
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
