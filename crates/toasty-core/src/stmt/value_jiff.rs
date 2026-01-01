use crate::stmt::Value;

macro_rules! impl_jiff_conversions {
    ($jiff:ty, $name:ident, $lit:literal) => {
        impl From<$jiff> for Value {
            fn from(value: $jiff) -> Self {
                Self::$name(value)
            }
        }

        impl TryFrom<Value> for $jiff {
            type Error = crate::Error;

            fn try_from(value: Value) -> Result<Self, Self::Error> {
                match value {
                    Value::$name(value) => Ok(value),
                    _ => Err(anyhow::anyhow!("value is not of type {}", $lit)),
                }
            }
        }
    };
}

impl_jiff_conversions!(jiff::Timestamp, JiffTimestamp, "Timestamp");
impl_jiff_conversions!(jiff::Zoned, JiffZoned, "Zoned");
impl_jiff_conversions!(jiff::civil::Date, JiffDate, "Date");
impl_jiff_conversions!(jiff::civil::Time, JiffTime, "Time");
impl_jiff_conversions!(jiff::civil::DateTime, JiffDateTime, "DateTime");
