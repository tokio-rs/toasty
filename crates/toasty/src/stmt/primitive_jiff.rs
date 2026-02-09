use crate::stmt::{Path, Primitive};
use toasty_core::{
    stmt::{Type, Value},
    Result,
};

macro_rules! impl_jiff_primitive {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl Primitive for $ty {
            type FieldAccessor = Path<Self>;

            fn ty() -> Type {
                Type::$name
            }

            fn load(value: Value) -> Result<Self> {
                match value {
                    Value::$name(v) => Ok(v),
                    _ => Err(toasty_core::Error::type_conversion(value, $lit)),
                }
            }

            fn make_field_accessor(path: Path<Self>) -> Self::FieldAccessor {
                path
            }
        }
    };
}

impl_jiff_primitive!(jiff::Timestamp, Timestamp, "jiff::Timestamp");
impl_jiff_primitive!(jiff::Zoned, Zoned, "jiff::Zoned");
impl_jiff_primitive!(jiff::civil::Date, Date, "jiff::civil::Date");
impl_jiff_primitive!(jiff::civil::Time, Time, "jiff::civil::Time");
impl_jiff_primitive!(jiff::civil::DateTime, DateTime, "jiff::civil::DateTime");
