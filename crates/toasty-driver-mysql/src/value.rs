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
                    let bytes: Vec<u8> = match row.take_opt(i).expect("value missing") {
                        Ok(v) => v,
                        Err(e) => {
                            assert!(matches!(e.0, mysql_async::Value::NULL));
                            return Value(stmt::Value::Null);
                        }
                    };
                    json_bytes_to_value_list(&bytes, elem)
                }
                _ => todo!("MySQL JSON column with stmt::Type {ty:#?}"),
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
            CoreValue::List(_) => {
                // Bound to a MySQL `JSON` column — serialize the list to a
                // JSON document and send it as text. MySQL accepts JSON as
                // string or bytes; bytes avoids any utf8 round-trip.
                mysql_async::Value::Bytes(value_list_to_json_bytes(&self.0))
            }
            value => todo!("{:#?}", value),
        }
    }
}

/// Encode a `Value::List` as a JSON document for a MySQL `JSON` column.
/// Element representation matches the column-level encoding: integers and
/// floats become JSON numbers, booleans become JSON booleans, strings /
/// UUIDs / decimals / timestamps become JSON strings. Panics on nested
/// lists, records, and bytes — none of those are valid `Vec<scalar>`
/// element types.
fn value_list_to_json_bytes(value: &CoreValue) -> Vec<u8> {
    let CoreValue::List(items) = value else {
        unreachable!("value_list_to_json_bytes called on {value:?}")
    };
    let items: Vec<serde_json::Value> = items.iter().map(scalar_to_json).collect();
    serde_json::to_vec(&serde_json::Value::Array(items)).expect("serializing scalar list to JSON")
}

/// Decode a JSON document fetched from a MySQL `JSON` column into a
/// `Value::List`. The element type comes from the schema and drives
/// per-element decoding so values round-trip cleanly.
fn json_bytes_to_value_list(bytes: &[u8], elem_ty: &stmt::Type) -> CoreValue {
    let json: serde_json::Value =
        serde_json::from_slice(bytes).expect("MySQL returned non-JSON for a JSON column");
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
        #[cfg(feature = "rust_decimal")]
        CoreValue::Decimal(v) => J::String(v.to_string()),
        #[cfg(feature = "jiff")]
        CoreValue::Timestamp(v) => J::String(v.to_string()),
        #[cfg(feature = "jiff")]
        CoreValue::Date(v) => J::String(v.to_string()),
        #[cfg(feature = "jiff")]
        CoreValue::Time(v) => J::String(v.to_string()),
        #[cfg(feature = "jiff")]
        CoreValue::DateTime(v) => J::String(v.to_string()),
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
        #[cfg(feature = "rust_decimal")]
        (stmt::Type::Decimal, J::String(v)) => {
            CoreValue::Decimal(v.parse().expect("invalid Decimal in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Timestamp, J::String(v)) => {
            CoreValue::Timestamp(v.parse().expect("invalid Timestamp in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Date, J::String(v)) => {
            CoreValue::Date(v.parse().expect("invalid Date in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::Time, J::String(v)) => {
            CoreValue::Time(v.parse().expect("invalid Time in JSON"))
        }
        #[cfg(feature = "jiff")]
        (stmt::Type::DateTime, J::String(v)) => {
            CoreValue::DateTime(v.parse().expect("invalid DateTime in JSON"))
        }
        (ty, json) => todo!("decode JSON value {json:?} as {ty:?}"),
    }
}
