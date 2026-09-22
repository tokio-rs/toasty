use sqlx_core::{
    arguments::Arguments, column::Column as _, decode::Decode, row::Row, type_info::TypeInfo as _,
    value::ValueRef as _,
};
use sqlx_mysql::{MySql, MySqlArguments, MySqlColumn, MySqlRow, types::MySqlTime};
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
    pub(crate) fn from_sql(
        i: usize,
        row: &MySqlRow,
        column: &MySqlColumn,
        ty: &stmt::Type,
    ) -> Result<Self, sqlx_core::Error> {
        if row.try_get_raw(i)?.is_null() {
            return Ok(Self(CoreValue::Null));
        }

        let value = match ty {
            stmt::Type::Bool => decode(row, i, CoreValue::Bool)?,
            stmt::Type::I8 => decode(row, i, CoreValue::I8)?,
            stmt::Type::I16 => decode(row, i, CoreValue::I16)?,
            stmt::Type::I32 => decode(row, i, CoreValue::I32)?,
            stmt::Type::I64 => decode(row, i, CoreValue::I64)?,
            stmt::Type::U8 => decode(row, i, CoreValue::U8)?,
            stmt::Type::U16 => decode(row, i, CoreValue::U16)?,
            stmt::Type::U32 => decode(row, i, CoreValue::U32)?,
            stmt::Type::U64 => decode(row, i, CoreValue::U64)?,
            stmt::Type::F32 => decode(row, i, CoreValue::F32)?,
            stmt::Type::F64 => decode(row, i, CoreValue::F64)?,
            stmt::Type::String => decode_string(row, i, column.type_info().name())?,
            stmt::Type::Bytes => decode(row, i, CoreValue::Bytes)?,
            stmt::Type::Uuid => decode_uuid(row, i)?,
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => map_string(decode_string(row, i, "DECIMAL")?, |value| {
                CoreValue::Decimal(value.parse().expect("failed to parse Decimal from MySQL"))
            }),
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => map_string(decode_string(row, i, "DECIMAL")?, |value| {
                CoreValue::BigDecimal(
                    value
                        .parse()
                        .expect("failed to parse BigDecimal from MySQL"),
                )
            }),
            #[cfg(feature = "jiff")]
            stmt::Type::Timestamp => map_datetime(decode_datetime(row, i)?, |value| {
                CoreValue::Timestamp(
                    value
                        .to_zoned(jiff::tz::TimeZone::UTC)
                        .expect("MySQL DATETIME is outside jiff's range")
                        .into(),
                )
            }),
            #[cfg(feature = "jiff")]
            stmt::Type::Date => decode_date(row, i)?,
            #[cfg(feature = "jiff")]
            stmt::Type::Time => decode_time(row, i)?,
            #[cfg(feature = "jiff")]
            stmt::Type::DateTime => decode_datetime(row, i)?,
            stmt::Type::List(elem) => decode_json(row, i, |bytes| {
                toasty_sql::json::list_from_slice(bytes, elem)
                    .expect("MySQL returned non-JSON for a JSON column")
            })?,
            stmt::Type::Object => decode_json(row, i, |bytes| {
                toasty_sql::json::from_slice(bytes, ty)
                    .expect("MySQL returned non-JSON for a JSON document column")
            })?,
            stmt::Type::Null => CoreValue::Null,
            _ => todo!(
                "implement SQLx MySQL conversion for `{}` and {ty:#?}",
                column.type_info().name()
            ),
        };

        Ok(Self(value))
    }

    /// Converts a MySQL value using the metadata returned by SQLx.
    pub(crate) fn from_sql_infer(
        i: usize,
        row: &MySqlRow,
        column: &MySqlColumn,
    ) -> Result<Self, sqlx_core::Error> {
        if row.try_get_raw(i)?.is_null() {
            return Ok(Self(CoreValue::Null));
        }

        let name = column.type_info().name();
        let value = match name {
            "TINYINT UNSIGNED" | "SMALLINT UNSIGNED" | "MEDIUMINT UNSIGNED" | "INT UNSIGNED"
            | "BIGINT UNSIGNED" | "YEAR" | "BIT" => decode(row, i, CoreValue::U64)?,
            "BOOLEAN" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => {
                decode(row, i, CoreValue::I64)?
            }
            "FLOAT" => decode(row, i, CoreValue::F32)?,
            "DOUBLE" => decode(row, i, CoreValue::F64)?,
            "DATE" => infer_date(row, i)?,
            "TIME" => infer_time(row, i)?,
            "TIMESTAMP" | "DATETIME" => infer_datetime(row, i)?,
            "NULL" => CoreValue::Null,
            "CHAR" | "VARCHAR" | "BINARY" | "VARBINARY" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT"
            | "LONGTEXT" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" | "ENUM" | "SET"
            | "DECIMAL" | "JSON" | "GEOMETRY" => infer_bytes(row, i)?,
            _ => todo!("infer Toasty type for SQLx MySQL type `{name}`"),
        };

        Ok(Self(value))
    }

    /// Adds an owned Toasty value to SQLx's MySQL argument buffer.
    pub(crate) fn add_to(self, args: &mut MySqlArguments) -> Result<(), sqlx_core::Error> {
        macro_rules! add {
            ($value:expr) => {
                Arguments::add(args, $value).map_err(sqlx_core::Error::Encode)
            };
        }

        match self.0 {
            CoreValue::Bool(value) => add!(value),
            CoreValue::I8(value) => add!(value),
            CoreValue::I16(value) => add!(value),
            CoreValue::I32(value) => add!(value),
            CoreValue::I64(value) => add!(value),
            CoreValue::U8(value) => add!(value),
            CoreValue::U16(value) => add!(value),
            CoreValue::U32(value) => add!(value),
            CoreValue::U64(value) => add!(value),
            CoreValue::F32(value) => add!(value),
            CoreValue::F64(value) => add!(value),
            CoreValue::Null => add!(Option::<String>::None),
            CoreValue::String(value) => add!(value),
            CoreValue::Bytes(value) => add!(value),
            CoreValue::Uuid(value) => add!(value.hyphenated().to_string()),
            #[cfg(feature = "rust_decimal")]
            CoreValue::Decimal(value) => add!(value.to_string()),
            #[cfg(feature = "bigdecimal")]
            CoreValue::BigDecimal(value) => add!(value.to_string()),
            #[cfg(feature = "jiff")]
            CoreValue::Timestamp(value) => add!(jiff_datetime_to_time(
                value.to_zoned(jiff::tz::TimeZone::UTC).datetime()
            )?),
            #[cfg(feature = "jiff")]
            CoreValue::Date(value) => add!(jiff_date_to_time(value)?),
            #[cfg(feature = "jiff")]
            CoreValue::Time(value) => add!(jiff_time_to_time(value)?),
            #[cfg(feature = "jiff")]
            CoreValue::DateTime(value) => add!(jiff_datetime_to_time(value)?),
            value @ (CoreValue::List(_) | CoreValue::Object(_)) => {
                let value = toasty_sql::json::to_vec(&value).expect("serialize MySQL JSON column");
                add!(String::from_utf8(value).expect("serialized JSON is valid UTF-8"))
            }
            value => todo!("encode Toasty value for SQLx MySQL: {value:#?}"),
        }
    }
}

fn decode<'r, T>(
    row: &'r MySqlRow,
    i: usize,
    constructor: impl FnOnce(T) -> CoreValue,
) -> Result<CoreValue, sqlx_core::Error>
where
    T: Decode<'r, MySql>,
{
    row.try_get_unchecked(i).map(constructor)
}

fn decode_string(row: &MySqlRow, i: usize, mysql_ty: &str) -> Result<CoreValue, sqlx_core::Error> {
    match mysql_ty {
        "DATE" => row
            .try_get_unchecked::<time::Date, _>(i)
            .map(|value| CoreValue::String(value.to_string())),
        "TIME" => row
            .try_get_unchecked::<MySqlTime, _>(i)
            .map(|value| CoreValue::String(value.to_string())),
        "TIMESTAMP" | "DATETIME" => row
            .try_get_unchecked::<time::PrimitiveDateTime, _>(i)
            .map(|value| CoreValue::String(value.to_string())),
        _ => row.try_get_unchecked::<Vec<u8>, _>(i).map(|bytes| {
            CoreValue::String(
                String::from_utf8(bytes).expect("invalid UTF-8 in MySQL string value"),
            )
        }),
    }
}

fn decode_uuid(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<Vec<u8>, _>(i).map(|bytes| {
        let uuid = uuid::Uuid::from_slice(&bytes).or_else(|_| {
            let value = std::str::from_utf8(&bytes).expect("invalid UTF-8 in MySQL UUID value");
            uuid::Uuid::parse_str(value)
        });
        CoreValue::Uuid(uuid.expect("failed to parse UUID from MySQL value"))
    })
}

fn decode_json(
    row: &MySqlRow,
    i: usize,
    constructor: impl FnOnce(&[u8]) -> CoreValue,
) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<Vec<u8>, _>(i)
        .map(|bytes| constructor(&bytes))
}

fn infer_bytes(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<Vec<u8>, _>(i).map(|bytes| {
        String::from_utf8(bytes)
            .map(CoreValue::String)
            .unwrap_or_else(|error| CoreValue::Bytes(error.into_bytes()))
    })
}

#[cfg(feature = "jiff")]
fn decode_date(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<time::Date, _>(i).map(|value| {
        CoreValue::Date(jiff::civil::Date::constant(
            value
                .year()
                .try_into()
                .expect("MySQL year outside jiff's range"),
            u8::from(value.month()) as i8,
            value.day() as i8,
        ))
    })
}

#[cfg(feature = "jiff")]
fn decode_time(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<time::Time, _>(i).map(|value| {
        CoreValue::Time(jiff::civil::Time::constant(
            value.hour() as i8,
            value.minute() as i8,
            value.second() as i8,
            value.nanosecond() as i32,
        ))
    })
}

#[cfg(feature = "jiff")]
fn decode_datetime(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    row.try_get_unchecked::<time::PrimitiveDateTime, _>(i)
        .map(|value| {
            CoreValue::DateTime(jiff::civil::DateTime::constant(
                value
                    .year()
                    .try_into()
                    .expect("MySQL year outside jiff's range"),
                u8::from(value.month()) as i8,
                value.day() as i8,
                value.hour() as i8,
                value.minute() as i8,
                value.second() as i8,
                value.nanosecond() as i32,
            ))
        })
}

#[cfg(feature = "jiff")]
fn infer_date(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_date(row, i)
}

#[cfg(not(feature = "jiff"))]
fn infer_date(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_string(row, i, "DATE")
}

#[cfg(feature = "jiff")]
fn infer_time(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_time(row, i)
}

#[cfg(not(feature = "jiff"))]
fn infer_time(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_string(row, i, "TIME")
}

#[cfg(feature = "jiff")]
fn infer_datetime(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_datetime(row, i)
}

#[cfg(not(feature = "jiff"))]
fn infer_datetime(row: &MySqlRow, i: usize) -> Result<CoreValue, sqlx_core::Error> {
    decode_string(row, i, "DATETIME")
}

#[cfg(feature = "jiff")]
fn jiff_date_to_time(value: jiff::civil::Date) -> Result<time::Date, sqlx_core::Error> {
    time::Date::from_calendar_date(
        value.year().into(),
        time::Month::try_from(value.month() as u8).map_err(encode_error)?,
        value.day() as u8,
    )
    .map_err(encode_error)
}

#[cfg(feature = "jiff")]
fn jiff_time_to_time(value: jiff::civil::Time) -> Result<time::Time, sqlx_core::Error> {
    time::Time::from_hms_nano(
        value.hour() as u8,
        value.minute() as u8,
        value.second() as u8,
        value.subsec_nanosecond() as u32,
    )
    .map_err(encode_error)
}

#[cfg(feature = "jiff")]
fn jiff_datetime_to_time(
    value: jiff::civil::DateTime,
) -> Result<time::PrimitiveDateTime, sqlx_core::Error> {
    Ok(time::PrimitiveDateTime::new(
        jiff_date_to_time(value.date())?,
        jiff_time_to_time(value.time())?,
    ))
}

#[cfg(feature = "jiff")]
fn encode_error(error: impl std::error::Error + Send + Sync + 'static) -> sqlx_core::Error {
    sqlx_core::Error::Encode(Box::new(error))
}

#[cfg(any(feature = "rust_decimal", feature = "bigdecimal"))]
fn map_string(value: CoreValue, constructor: impl FnOnce(String) -> CoreValue) -> CoreValue {
    match value {
        CoreValue::String(value) => constructor(value),
        value => value,
    }
}

#[cfg(feature = "jiff")]
fn map_datetime(
    value: CoreValue,
    constructor: impl FnOnce(jiff::civil::DateTime) -> CoreValue,
) -> CoreValue {
    match value {
        CoreValue::DateTime(value) => constructor(value),
        value => value,
    }
}
