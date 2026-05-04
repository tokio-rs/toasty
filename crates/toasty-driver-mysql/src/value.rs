use mysql_async::{Column, Row, prelude::ToValue};
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

            CT::MYSQL_TYPE_FLOAT => match ty {
                stmt::Type::F32 => extract_or_null(row, i, stmt::Value::F32),
                stmt::Type::F64 => extract_or_null(row, i, |v: f32| stmt::Value::F64(v as f64)),
                _ => todo!("ty={ty:#?}"),
            },

            CT::MYSQL_TYPE_DOUBLE => match ty {
                stmt::Type::F64 => extract_or_null(row, i, stmt::Value::F64),
                stmt::Type::F32 => extract_or_null(row, i, |v: f64| stmt::Value::F32(v as f32)),
                _ => todo!("ty={ty:#?}"),
            },

            CT::MYSQL_TYPE_JSON => match ty {
                stmt::Type::List(elem) => {
                    match row.take_opt::<String, _>(i).expect("value missing") {
                        Ok(json) => decode_json_list(&json, elem),
                        Err(e) => {
                            assert!(matches!(e.0, mysql_async::Value::NULL));
                            stmt::Value::Null
                        }
                    }
                }
                _ => todo!("unexpected type for JSON: {ty:#?}"),
            },

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
            CoreValue::F32(value) => value.to_value(),
            CoreValue::F64(value) => value.to_value(),
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
            CoreValue::List(items) => {
                // MySQL `JSON` columns accept a JSON-encoded text payload.
                // Whole-list reads round-trip through `decode_json_list` on
                // the read path.
                let json = serde_json::Value::Array(items.iter().map(value_to_json).collect());
                json.to_string().to_value()
            }
            value => todo!("{:#?}", value),
        }
    }
}

/// Decode a MySQL `JSON` payload (always a JSON array for a `Vec<scalar>`
/// column) into `Value::List` whose elements have type `elem`.
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
