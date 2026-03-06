use mysql_async::{prelude::ToValue, Column, Row};
use toasty_core::stmt::{self, Value as CoreValue};

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
    }
}

impl Value {
    /// Converts this MySQL driver value into the core Toasty value.
    pub fn into_inner(self) -> CoreValue {
        self.0
    }

    /// Converts a MySQL value within a row to a Toasty value.
    pub fn from_sql(i: usize, row: &mut Row, column: &Column, ty: &stmt::Type) -> Self {
        use mysql_async::consts::ColumnType as CT;

        /// Helper function to extract a value from a MySQL row or return Null if the value is NULL
        fn extract_or_null<T>(
            row: &mut Row,
            i: usize,
            constructor: fn(T) -> stmt::Value,
        ) -> stmt::Value
        where
            T: mysql_async::prelude::FromValue,
        {
            match row.take_opt(i).expect("value missing") {
                Ok(v) => constructor(v),
                Err(e) => {
                    assert!(matches!(e.0, mysql_async::Value::NULL));
                    stmt::Value::Null
                }
            }
        }

        let core_value = match column.column_type() {
            CT::MYSQL_TYPE_NULL => stmt::Value::Null,

            CT::MYSQL_TYPE_VARCHAR
            | CT::MYSQL_TYPE_VAR_STRING
            | CT::MYSQL_TYPE_STRING
            | CT::MYSQL_TYPE_BLOB => match ty {
                stmt::Type::String => extract_or_null(row, i, stmt::Value::String),
                stmt::Type::Uuid => extract_or_null(row, i, stmt::Value::Uuid),
                stmt::Type::Bytes => extract_or_null(row, i, stmt::Value::Bytes),
                _ => todo!("ty={ty:#?}"),
            },

            CT::MYSQL_TYPE_TINY
            | CT::MYSQL_TYPE_SHORT
            | CT::MYSQL_TYPE_INT24
            | CT::MYSQL_TYPE_LONG
            | CT::MYSQL_TYPE_LONGLONG => match ty {
                stmt::Type::Bool => extract_or_null(row, i, stmt::Value::Bool),
                stmt::Type::I8 => extract_or_null(row, i, stmt::Value::I8),
                stmt::Type::I16 => extract_or_null(row, i, stmt::Value::I16),
                stmt::Type::I32 => extract_or_null(row, i, stmt::Value::I32),
                stmt::Type::I64 => extract_or_null(row, i, stmt::Value::I64),
                stmt::Type::U8 => extract_or_null(row, i, stmt::Value::U8),
                stmt::Type::U16 => extract_or_null(row, i, stmt::Value::U16),
                stmt::Type::U32 => extract_or_null(row, i, stmt::Value::U32),
                stmt::Type::U64 => extract_or_null(row, i, stmt::Value::U64),
                _ => todo!("ty={ty:#?}"),
            },

            #[cfg(feature = "jiff")]
            CT::MYSQL_TYPE_TIMESTAMP | CT::MYSQL_TYPE_DATETIME => {
                match row.take_opt(i).expect("value missing") {
                    Ok(mysql_async::Value::Date(
                        year,
                        month,
                        day,
                        hour,
                        minute,
                        second,
                        microsecond,
                    )) => {
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
                            stmt::Type::Timestamp => stmt::Value::Timestamp(
                                dt.to_zoned(jiff::tz::TimeZone::UTC).unwrap().into(),
                            ),
                            _ => todo!("unexpected type for DATETIME: {ty:#?}"),
                        }
                    }
                    Ok(mysql_async::Value::NULL) | Err(_) => stmt::Value::Null,
                    Ok(v) => panic!("unexpected MySQL value for TIMESTAMP/DATETIME: {v:#?}"),
                }
            }

            #[cfg(feature = "jiff")]
            CT::MYSQL_TYPE_DATE => match row.take_opt(i).expect("value missing") {
                Ok(mysql_async::Value::Date(year, month, day, _, _, _, _)) => stmt::Value::Date(
                    jiff::civil::Date::constant(year as i16, month as i8, day as i8),
                ),
                Ok(mysql_async::Value::NULL) | Err(_) => stmt::Value::Null,
                Ok(v) => panic!("unexpected MySQL value for DATE: {v:#?}"),
            },

            #[cfg(feature = "jiff")]
            CT::MYSQL_TYPE_TIME => {
                match row.take_opt(i).expect("value missing") {
                    Ok(mysql_async::Value::Time(
                        _is_negative,
                        _days,
                        hour,
                        minute,
                        second,
                        microsecond,
                    )) => {
                        stmt::Value::Time(jiff::civil::Time::constant(
                            hour as i8,
                            minute as i8,
                            second as i8,
                            (microsecond * 1000) as i32, // Convert microseconds to nanoseconds
                        ))
                    }
                    Ok(mysql_async::Value::NULL) | Err(_) => stmt::Value::Null,
                    Ok(v) => panic!("unexpected MySQL value for TIME: {v:#?}"),
                }
            }

            CT::MYSQL_TYPE_NEWDECIMAL | CT::MYSQL_TYPE_DECIMAL => match ty {
                #[cfg(feature = "rust_decimal")]
                stmt::Type::Decimal => extract_or_null(row, i, |s: String| {
                    stmt::Value::Decimal(s.parse().expect("failed to parse Decimal from MySQL"))
                }),
                #[cfg(feature = "bigdecimal")]
                stmt::Type::BigDecimal => extract_or_null(row, i, |s: String| {
                    stmt::Value::BigDecimal(
                        s.parse().expect("failed to parse BigDecimal from MySQL"),
                    )
                }),
                _ => todo!("unexpected type for DECIMAL: {ty:#?}"),
            },

            _ => todo!(
                "implement MySQL to toasty conversion for `{:#?}`; {:#?}; ty={:#?}",
                column.column_type(),
                row.get::<mysql_async::Value, _>(i),
                ty
            ),
        };

        Value(core_value)
    }
}

impl ToValue for Value {
    fn to_value(&self) -> mysql_async::Value {
        match &self.0 {
            CoreValue::Bool(value) => value.to_value(),
            CoreValue::I8(value) => value.to_value(),
            CoreValue::I16(value) => value.to_value(),
            CoreValue::I32(value) => value.to_value(),
            CoreValue::I64(value) => value.to_value(),
            CoreValue::U8(value) => value.to_value(),
            CoreValue::U16(value) => value.to_value(),
            CoreValue::U32(value) => value.to_value(),
            CoreValue::U64(value) => value.to_value(),
            CoreValue::Null => mysql_async::Value::NULL,
            CoreValue::String(value) => value.to_value(),
            CoreValue::Bytes(value) => value.to_value(),
            CoreValue::Uuid(value) => value.to_value(),
            #[cfg(feature = "rust_decimal")]
            CoreValue::Decimal(value) => value.to_string().to_value(),
            #[cfg(feature = "bigdecimal")]
            CoreValue::BigDecimal(value) => value.to_string().to_value(),
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
            value => todo!("{:#?}", value),
        }
    }
}
