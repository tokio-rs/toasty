use mysql_async::prelude::ToValue;
use toasty_core::stmt::Value as CoreValue;

#[derive(Debug)]
pub struct Value(CoreValue);

impl From<CoreValue> for Value {
    fn from(value: CoreValue) -> Self {
        Self(value)
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
            CoreValue::Id(id) => id.to_string().to_value(),
            CoreValue::Null => mysql_async::Value::NULL,
            CoreValue::String(value) => value.to_value(),
            CoreValue::Bytes(value) => value.to_value(),
            CoreValue::Uuid(value) => value.to_value(),
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
