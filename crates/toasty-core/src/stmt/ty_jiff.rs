use jiff::tz::TimeZone;

use crate::{
    stmt::{Type, Value},
    Result,
};

impl Type {
    pub fn cast_jiff(&self, value: &Value) -> Result<Option<Value>> {
        Ok(Some(match (value, self) {
            // String -> jiff
            (Value::String(value), Type::Timestamp) => {
                let v = value.clone();
                Value::Timestamp(
                    value.parse().map_err(|_| {
                        crate::Error::type_conversion(Value::String(v), "Timestamp")
                    })?,
                )
            }
            (Value::String(value), Type::Zoned) => {
                let v = value.clone();
                Value::Zoned(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Zoned"))?,
                )
            }
            (Value::String(value), Type::Date) => {
                let v = value.clone();
                Value::Date(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Date"))?,
                )
            }
            (Value::String(value), Type::Time) => {
                let v = value.clone();
                Value::Time(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "Time"))?,
                )
            }
            (Value::String(value), Type::DateTime) => {
                let v = value.clone();
                Value::DateTime(
                    value
                        .parse()
                        .map_err(|_| crate::Error::type_conversion(Value::String(v), "DateTime"))?,
                )
            }

            // jiff -> String
            //
            // Types with sub-second precision use fixed 9-digit nanosecond
            // formatting so that the resulting strings sort lexicographically
            // in chronological order (ISO 8601 guarantees this for
            // uniform-precision representations).
            (Value::Timestamp(value), Type::String) => Value::String(format!("{value:.9}")),
            (Value::Zoned(value), Type::String) => Value::String(format!("{value:.9}")),
            (Value::Date(value), Type::String) => Value::String(value.to_string()),
            (Value::Time(value), Type::String) => Value::String(format!("{value:.9}")),
            (Value::DateTime(value), Type::String) => Value::String(format!("{value:.9}")),

            // UTC <-> Zoned
            (Value::Timestamp(value), Type::Zoned) => Value::Zoned(value.to_zoned(TimeZone::UTC)),
            (Value::Zoned(value), Type::Timestamp) => Value::Timestamp(value.into()),

            // UTC <-> Civil
            (Value::Timestamp(value), Type::DateTime) => {
                Value::DateTime(value.to_zoned(TimeZone::UTC).into())
            }
            (Value::DateTime(value), Type::Timestamp) => Value::Timestamp(
                value
                    .to_zoned(TimeZone::UTC)
                    .expect("value was too close to minimum or maximum DateTime")
                    .into(),
            ),

            // Zoned <-> Civil
            (Value::Zoned(value), Type::DateTime) => Value::DateTime(value.into()),
            (Value::DateTime(value), Type::Zoned) => Value::Zoned(
                value
                    .to_zoned(TimeZone::UTC)
                    .expect("value was too close to minimum or maximum DateTime"),
            ),

            _ => return Ok(None),
        }))
    }
}
