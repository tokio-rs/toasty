use mysql_async::{Column, Row, prelude::ToValue};
use std::fmt;
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
pub(crate) struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this MySQL driver value into the core Toasty value.
    pub(crate) fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a MySQL value within a row to a Toasty value.
    pub(crate) fn from_sql(i: usize, row: &mut Row, column: &Column, ty: &stmt::Type) -> Self {
        let value = take_mysql_value(row, i);

        Value(typed_mysql_value_to_core(value, column, ty))
    }

    /// Converts a MySQL value within a row using the value metadata available
    /// from the driver.
    pub(crate) fn from_sql_infer(i: usize, row: &mut Row, column: &Column) -> Self {
        let value = take_mysql_value(row, i);
        let ty = infer_mysql_type(column, &value);

        Value(typed_mysql_value_to_core(value, column, &ty))
    }
}

fn take_mysql_value(row: &mut Row, i: usize) -> mysql_async::Value {
    match row.take_opt(i).expect("value missing") {
        Ok(value) => value,
        Err(err) => err.0,
    }
}

fn infer_mysql_type(column: &Column, value: &mysql_async::Value) -> stmt::Type {
    use mysql_async::consts::ColumnFlags as CF;

    match value {
        mysql_async::Value::NULL => stmt::Type::Null,
        mysql_async::Value::Bytes(bytes) => {
            if std::str::from_utf8(bytes).is_ok() {
                stmt::Type::String
            } else {
                stmt::Type::Bytes
            }
        }
        mysql_async::Value::Int(_) if column.flags().contains(CF::UNSIGNED_FLAG) => stmt::Type::U64,
        mysql_async::Value::Int(_) => stmt::Type::I64,
        mysql_async::Value::UInt(_) => stmt::Type::U64,
        mysql_async::Value::Float(_) => stmt::Type::F32,
        mysql_async::Value::Double(_) => stmt::Type::F64,
        mysql_async::Value::Date(_, _, _, 0, 0, 0, 0) => {
            #[cfg(feature = "jiff")]
            {
                stmt::Type::Date
            }
            #[cfg(not(feature = "jiff"))]
            {
                stmt::Type::String
            }
        }
        mysql_async::Value::Date(..) => {
            #[cfg(feature = "jiff")]
            {
                stmt::Type::DateTime
            }
            #[cfg(not(feature = "jiff"))]
            {
                stmt::Type::String
            }
        }
        mysql_async::Value::Time(..) => {
            #[cfg(feature = "jiff")]
            {
                stmt::Type::Time
            }
            #[cfg(not(feature = "jiff"))]
            {
                stmt::Type::String
            }
        }
    }
}

fn typed_mysql_value_to_core(
    value: mysql_async::Value,
    column: &Column,
    ty: &stmt::Type,
) -> CoreValue {
    use mysql_async::consts::ColumnType as CT;

    if matches!(value, mysql_async::Value::NULL) {
        return stmt::Value::Null;
    }

    match column.column_type() {
        CT::MYSQL_TYPE_NULL => stmt::Value::Null,

        CT::MYSQL_TYPE_VARCHAR
        | CT::MYSQL_TYPE_VAR_STRING
        | CT::MYSQL_TYPE_STRING
        | CT::MYSQL_TYPE_TINY_BLOB
        | CT::MYSQL_TYPE_MEDIUM_BLOB
        | CT::MYSQL_TYPE_LONG_BLOB
        | CT::MYSQL_TYPE_BLOB
        | CT::MYSQL_TYPE_ENUM
        | CT::MYSQL_TYPE_SET => match ty {
            stmt::Type::String => bytes_to_string_value(value),
            stmt::Type::Uuid => bytes_to_uuid_value(value),
            stmt::Type::Bytes => bytes_to_bytes_value(value),
            _ => todo!("ty={ty:#?}"),
        },

        CT::MYSQL_TYPE_TINY
        | CT::MYSQL_TYPE_SHORT
        | CT::MYSQL_TYPE_INT24
        | CT::MYSQL_TYPE_LONG
        | CT::MYSQL_TYPE_LONGLONG => match ty {
            stmt::Type::Bool => integer_to_bool_value(value),
            stmt::Type::I8 => integer_to_signed_value(value, stmt::Value::I8, "i8"),
            stmt::Type::I16 => integer_to_signed_value(value, stmt::Value::I16, "i16"),
            stmt::Type::I32 => integer_to_signed_value(value, stmt::Value::I32, "i32"),
            stmt::Type::I64 => integer_to_signed_value(value, stmt::Value::I64, "i64"),
            stmt::Type::U8 => integer_to_unsigned_value(value, stmt::Value::U8, "u8"),
            stmt::Type::U16 => integer_to_unsigned_value(value, stmt::Value::U16, "u16"),
            stmt::Type::U32 => integer_to_unsigned_value(value, stmt::Value::U32, "u32"),
            stmt::Type::U64 => integer_to_unsigned_value(value, stmt::Value::U64, "u64"),
            _ => todo!("ty={ty:#?}"),
        },

        #[cfg(feature = "jiff")]
        CT::MYSQL_TYPE_TIMESTAMP | CT::MYSQL_TYPE_DATETIME => match value {
            mysql_async::Value::Date(year, month, day, hour, minute, second, microsecond) => {
                let dt = jiff::civil::DateTime::constant(
                    year as i16,
                    month as i8,
                    day as i8,
                    hour as i8,
                    minute as i8,
                    second as i8,
                    (microsecond * 1000) as i32, // Convert microseconds to nanoseconds
                );
                match ty {
                    stmt::Type::DateTime => stmt::Value::DateTime(dt),
                    stmt::Type::Timestamp => {
                        stmt::Value::Timestamp(dt.to_zoned(jiff::tz::TimeZone::UTC).unwrap().into())
                    }
                    _ => todo!("unexpected type for DATETIME: {ty:#?}"),
                }
            }
            mysql_async::Value::NULL => stmt::Value::Null,
            v => panic!("unexpected MySQL value for TIMESTAMP/DATETIME: {v:#?}"),
        },

        #[cfg(feature = "jiff")]
        CT::MYSQL_TYPE_DATE => match value {
            mysql_async::Value::Date(year, month, day, _, _, _, _) => stmt::Value::Date(
                jiff::civil::Date::constant(year as i16, month as i8, day as i8),
            ),
            mysql_async::Value::NULL => stmt::Value::Null,
            v => panic!("unexpected MySQL value for DATE: {v:#?}"),
        },

        #[cfg(feature = "jiff")]
        CT::MYSQL_TYPE_TIME => match value {
            mysql_async::Value::Time(_is_negative, _days, hour, minute, second, microsecond) => {
                stmt::Value::Time(jiff::civil::Time::constant(
                    hour as i8,
                    minute as i8,
                    second as i8,
                    (microsecond * 1000) as i32, // Convert microseconds to nanoseconds
                ))
            }
            mysql_async::Value::NULL => stmt::Value::Null,
            v => panic!("unexpected MySQL value for TIME: {v:#?}"),
        },

        #[cfg(not(feature = "jiff"))]
        CT::MYSQL_TYPE_TIMESTAMP | CT::MYSQL_TYPE_DATETIME | CT::MYSQL_TYPE_DATE => match ty {
            stmt::Type::String => match value {
                mysql_async::Value::Date(year, month, day, hour, minute, second, microsecond) => {
                    stmt::Value::String(format!(
                        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{microsecond:06}"
                    ))
                }
                v => panic!("unexpected MySQL value for TIMESTAMP/DATETIME/DATE: {v:#?}"),
            },
            _ => todo!("unexpected type for DATETIME: {ty:#?}"),
        },

        #[cfg(not(feature = "jiff"))]
        CT::MYSQL_TYPE_TIME => match ty {
            stmt::Type::String => match value {
                mysql_async::Value::Time(_, days, hour, minute, second, microsecond) => {
                    stmt::Value::String(format!(
                        "{days} {hour:02}:{minute:02}:{second:02}.{microsecond:06}"
                    ))
                }
                v => panic!("unexpected MySQL value for TIME: {v:#?}"),
            },
            _ => todo!("unexpected type for TIME: {ty:#?}"),
        },

        CT::MYSQL_TYPE_FLOAT => match ty {
            stmt::Type::F32 => convert_or_null(value, stmt::Value::F32),
            stmt::Type::F64 => convert_or_null(value, |v: f32| stmt::Value::F64(v as f64)),
            _ => todo!("ty={ty:#?}"),
        },

        CT::MYSQL_TYPE_DOUBLE => match ty {
            stmt::Type::F64 => convert_or_null(value, stmt::Value::F64),
            stmt::Type::F32 => convert_or_null(value, |v: f64| stmt::Value::F32(v as f32)),
            _ => todo!("ty={ty:#?}"),
        },

        CT::MYSQL_TYPE_JSON => match ty {
            // `Json<T>` is opaque to the engine. Reuse the protocol byte
            // buffer as a String, then let `Json<T>::load` deserialize it
            // directly into T.
            stmt::Type::String => bytes_to_string_value(value),
            stmt::Type::List(elem) => convert_or_null(value, |bytes: Vec<u8>| {
                json_bytes_to_value_list(&bytes, elem)
            }),
            // A `#[document]` column (`Type::Object`): decode the JSON object
            // shape-directed to the named `Value::Object` wire form; the
            // engine raises it to the embed's positional record.
            stmt::Type::Object => {
                convert_or_null(value, |bytes: Vec<u8>| json_bytes_to_value(&bytes, ty))
            }
            _ => todo!("MySQL JSON column with stmt::Type {ty:#?}"),
        },

        CT::MYSQL_TYPE_NEWDECIMAL | CT::MYSQL_TYPE_DECIMAL => match ty {
            stmt::Type::String => convert_or_null(value, stmt::Value::String),
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => convert_or_null(value, |s: String| {
                stmt::Value::Decimal(s.parse().expect("failed to parse Decimal from MySQL"))
            }),
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => convert_or_null(value, |s: String| {
                stmt::Value::BigDecimal(s.parse().expect("failed to parse BigDecimal from MySQL"))
            }),
            _ => todo!("unexpected type for DECIMAL: {ty:#?}"),
        },

        _ => todo!(
            "implement MySQL to toasty conversion for `{:#?}`; {:#?}; ty={:#?}",
            column.column_type(),
            value,
            ty
        ),
    }
}

fn bytes_to_string_value(value: mysql_async::Value) -> stmt::Value {
    match value {
        mysql_async::Value::Bytes(bytes) => stmt::Value::String(
            String::from_utf8(bytes).expect("invalid UTF-8 in MySQL string value"),
        ),
        mysql_async::Value::NULL => stmt::Value::Null,
        value => panic!("unexpected MySQL value for string: {value:#?}"),
    }
}

fn bytes_to_uuid_value(value: mysql_async::Value) -> stmt::Value {
    match value {
        mysql_async::Value::Bytes(bytes) => {
            let uuid = uuid::Uuid::from_slice(&bytes).or_else(|_| {
                let s = std::str::from_utf8(&bytes).expect("invalid UTF-8 in MySQL UUID value");
                uuid::Uuid::parse_str(s)
            });

            stmt::Value::Uuid(uuid.expect("failed to parse UUID from MySQL value"))
        }
        mysql_async::Value::NULL => stmt::Value::Null,
        value => panic!("unexpected MySQL value for UUID: {value:#?}"),
    }
}

fn bytes_to_bytes_value(value: mysql_async::Value) -> stmt::Value {
    match value {
        mysql_async::Value::Bytes(bytes) => stmt::Value::Bytes(bytes),
        mysql_async::Value::NULL => stmt::Value::Null,
        value => panic!("unexpected MySQL value for bytes: {value:#?}"),
    }
}

fn integer_to_bool_value(value: mysql_async::Value) -> stmt::Value {
    match integer_to_u128(value) {
        Some(value) => stmt::Value::Bool(value != 0),
        None => stmt::Value::Null,
    }
}

fn integer_to_signed_value<T>(
    value: mysql_async::Value,
    constructor: impl FnOnce(T) -> stmt::Value,
    ty: &str,
) -> stmt::Value
where
    T: TryFrom<i128>,
    T::Error: fmt::Debug,
{
    match integer_to_i128(value) {
        Some(value) => constructor(
            value
                .try_into()
                .unwrap_or_else(|_| panic!("MySQL {ty} value out of range")),
        ),
        None => stmt::Value::Null,
    }
}

fn integer_to_unsigned_value<T>(
    value: mysql_async::Value,
    constructor: impl FnOnce(T) -> stmt::Value,
    ty: &str,
) -> stmt::Value
where
    T: TryFrom<u128>,
    T::Error: fmt::Debug,
{
    match integer_to_u128(value) {
        Some(value) => constructor(
            value
                .try_into()
                .unwrap_or_else(|_| panic!("MySQL {ty} value out of range")),
        ),
        None => stmt::Value::Null,
    }
}

fn integer_to_i128(value: mysql_async::Value) -> Option<i128> {
    match value {
        mysql_async::Value::Int(value) => Some(value as i128),
        mysql_async::Value::UInt(value) => Some(value as i128),
        mysql_async::Value::NULL => None,
        value => panic!("unexpected MySQL integer value: {value:#?}"),
    }
}

fn integer_to_u128(value: mysql_async::Value) -> Option<u128> {
    match value {
        mysql_async::Value::Int(value) => Some(value.try_into().expect("negative MySQL integer")),
        mysql_async::Value::UInt(value) => Some(value as u128),
        mysql_async::Value::NULL => None,
        value => panic!("unexpected MySQL integer value: {value:#?}"),
    }
}

fn convert_or_null<T, F>(value: mysql_async::Value, constructor: F) -> stmt::Value
where
    T: mysql_async::prelude::FromValue,
    F: FnOnce(T) -> stmt::Value,
{
    match mysql_async::from_value_opt::<T>(value) {
        Ok(v) => constructor(v),
        Err(e) => {
            assert!(matches!(e.0, mysql_async::Value::NULL));
            stmt::Value::Null
        }
    }
}

impl Value {
    /// Converts an owned Toasty value into an owned MySQL parameter.
    ///
    /// String and byte buffers move into the MySQL value without another
    /// allocation. This matters for `Json<T>`, whose string is already the
    /// final serialized JSON representation.
    pub(crate) fn into_mysql(self) -> mysql_async::Value {
        match self.0 {
            CoreValue::Bool(value) => value.to_value(),
            CoreValue::I8(value) => value.to_value(),
            CoreValue::I16(value) => value.to_value(),
            CoreValue::I32(value) => value.to_value(),
            CoreValue::I64(value) => value.to_value(),
            CoreValue::U8(value) => value.to_value(),
            CoreValue::U16(value) => value.to_value(),
            CoreValue::U32(value) => value.to_value(),
            CoreValue::U64(value) => value.to_value(),
            CoreValue::F32(value) => value.to_value(),
            CoreValue::F64(value) => value.to_value(),
            CoreValue::Null => mysql_async::Value::NULL,
            CoreValue::String(value) => mysql_async::Value::Bytes(value.into_bytes()),
            CoreValue::Bytes(value) => mysql_async::Value::Bytes(value),
            CoreValue::Uuid(value) => value.to_value(),
            #[cfg(feature = "rust_decimal")]
            CoreValue::Decimal(value) => mysql_async::Value::Bytes(value.to_string().into_bytes()),
            #[cfg(feature = "bigdecimal")]
            CoreValue::BigDecimal(value) => {
                mysql_async::Value::Bytes(value.to_string().into_bytes())
            }
            #[cfg(feature = "jiff")]
            CoreValue::Timestamp(value) => {
                // Convert jiff::Timestamp to MySQL TIMESTAMP
                let dt = value.to_zoned(jiff::tz::TimeZone::UTC).datetime();
                mysql_async::Value::Date(
                    dt.year() as u16,
                    dt.month() as u8,
                    dt.day() as u8,
                    dt.hour() as u8,
                    dt.minute() as u8,
                    dt.second() as u8,
                    (dt.subsec_nanosecond() / 1000) as u32, // Convert nanoseconds to microseconds
                )
            }
            #[cfg(feature = "jiff")]
            CoreValue::Date(value) => mysql_async::Value::Date(
                value.year() as u16,
                value.month() as u8,
                value.day() as u8,
                0,
                0,
                0,
                0,
            ),
            #[cfg(feature = "jiff")]
            CoreValue::Time(value) => {
                mysql_async::Value::Time(
                    false, // is_negative
                    0,     // days
                    value.hour() as u8,
                    value.minute() as u8,
                    value.second() as u8,
                    (value.subsec_nanosecond() / 1000) as u32, // Convert nanoseconds to microseconds
                )
            }
            #[cfg(feature = "jiff")]
            CoreValue::DateTime(value) => {
                mysql_async::Value::Date(
                    value.year() as u16,
                    value.month() as u8,
                    value.day() as u8,
                    value.hour() as u8,
                    value.minute() as u8,
                    value.second() as u8,
                    (value.subsec_nanosecond() / 1000) as u32, // Convert nanoseconds to microseconds
                )
            }
            value @ CoreValue::List(_) => {
                // Bound to a MySQL `JSON` column — a `Vec<scalar>` field or a
                // `#[document]` collection. Serialize the list to a JSON
                // document and send it as bytes. MySQL accepts JSON as string
                // or bytes; bytes avoids any utf8 round-trip.
                mysql_async::Value::Bytes(
                    toasty_sql::json::to_vec(&value).expect("serialize JSON list column"),
                )
            }
            value @ CoreValue::Object(_) => {
                // Bound to a MySQL `JSON` column — a bare `#[document]` embed
                // (`Value::Object`). Serialize the named object to JSON bytes,
                // the same way the list arm handles a document collection.
                mysql_async::Value::Bytes(
                    toasty_sql::json::to_vec(&value).expect("serialize JSON document column"),
                )
            }
            value => todo!("{:#?}", value),
        }
    }
}

fn json_bytes_to_value_list(bytes: &[u8], elem_ty: &stmt::Type) -> CoreValue {
    toasty_sql::json::list_from_slice(bytes, elem_ty)
        .expect("MySQL returned non-JSON for a JSON column")
}

/// Decode the JSON bytes of a `#[document]` column (`Type::Object`) into the
/// named `Value::Object` wire form the engine raises.
fn json_bytes_to_value(bytes: &[u8], ty: &stmt::Type) -> CoreValue {
    toasty_sql::json::from_slice(bytes, ty)
        .expect("MySQL returned non-JSON for a JSON document column")
}
