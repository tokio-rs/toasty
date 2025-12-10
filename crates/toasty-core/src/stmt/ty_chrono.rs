use crate::{
    stmt::{Type, Value},
    Result,
};

impl Type {
    pub(crate) fn cast_chrono(&self, value: &Value) -> Result<Option<Value>> {
        Ok(Some(match (value, self) {
            // String -> chrono
            (Value::String(value), Type::ChronoDateTimeUtc) => {
                Value::ChronoDateTimeUtc(value.parse()?)
            }
            (Value::String(value), Type::ChronoNaiveDateTime) => {
                Value::ChronoNaiveDateTime(value.parse()?)
            }
            (Value::String(value), Type::ChronoNaiveDate) => Value::ChronoNaiveDate(value.parse()?),
            (Value::String(value), Type::ChronoNaiveTime) => Value::ChronoNaiveTime(value.parse()?),

            // chrono -> String
            (Value::ChronoDateTimeUtc(value), Type::String) => Value::String(value.to_string()),
            (Value::ChronoNaiveDateTime(value), Type::String) => {
                Value::String(value.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
            }
            (Value::ChronoNaiveDate(value), Type::String) => Value::String(value.to_string()),
            (Value::ChronoNaiveTime(value), Type::String) => Value::String(value.to_string()),

            _ => return Ok(None),
        }))
    }
}
