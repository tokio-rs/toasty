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
                    _ => Err(crate::Error::type_conversion(value, $lit)),
                }
            }
        }
    };
}

impl_jiff_conversions!(jiff::Timestamp, Timestamp, "Timestamp");
impl_jiff_conversions!(jiff::Zoned, Zoned, "Zoned");
impl_jiff_conversions!(jiff::civil::Date, Date, "Date");
impl_jiff_conversions!(jiff::civil::Time, Time, "Time");
impl_jiff_conversions!(jiff::civil::DateTime, DateTime, "DateTime");
