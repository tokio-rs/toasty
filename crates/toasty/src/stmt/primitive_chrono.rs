use crate::stmt::Primitive;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use toasty_core::{
    stmt::{Type, Value},
    Result,
};

macro_rules! impl_chrono_primitive {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl Primitive for $ty {
            fn ty() -> Type {
                Type::$name
            }

            fn load(value: Value) -> Result<Self> {
                match value {
                    Value::$name(v) => Ok(v),
                    _ => anyhow::bail!("cannot convert value to {} {value:#?}", $lit),
                }
            }
        }
    };
}

impl_chrono_primitive!(DateTime<Utc>, ChronoDateTimeUtc, "chrono::DateTime<Utc>");
impl_chrono_primitive!(NaiveDateTime, ChronoNaiveDateTime, "chrono::NaiveDateTime");
impl_chrono_primitive!(NaiveDate, ChronoNaiveDate, "chrono::NaiveDate");
impl_chrono_primitive!(NaiveTime, ChronoNaiveTime, "chrono::NaiveTime");
