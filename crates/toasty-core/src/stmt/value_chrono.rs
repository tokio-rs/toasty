use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use crate::stmt::Value;

macro_rules! impl_chrono_conversions {
    ($chrono:ty, $name:ident, $lit:literal) => {
        impl From<$chrono> for Value {
            fn from(value: $chrono) -> Self {
                Self::$name(value)
            }
        }

        impl TryFrom<Value> for $chrono {
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

impl_chrono_conversions!(DateTime<Utc>, ChronoDateTimeUtc, "DateTime<Utc>");
impl_chrono_conversions!(NaiveDateTime, ChronoNaiveDateTime, "NaiveDateTime");
impl_chrono_conversions!(NaiveDate, ChronoNaiveDate, "NaiveDate");
impl_chrono_conversions!(NaiveTime, ChronoNaiveTime, "NaiveTime");
