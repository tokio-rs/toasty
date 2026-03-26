use super::{Load, ModelField, Scope};
use crate::stmt::Path;
use toasty_core::{
    stmt::{Type, Value},
    Result,
};

macro_rules! impl_jiff_field {
    ($ty:ty, $name:ident, $lit:literal) => {
        impl Load for $ty {
            type Output = Self;

            fn ty() -> Type {
                Type::$name
            }

            fn load(value: Value) -> Result<Self> {
                match value {
                    Value::$name(v) => Ok(v),
                    _ => Err(toasty_core::Error::type_conversion(value, $lit)),
                }
            }

            fn reload(target: &mut Self, value: Value) -> Result<()> {
                *target = Self::load(value)?;
                Ok(())
            }
        }

        impl ModelField for $ty {
            type Path<Origin> = Path<Origin, Self>;

            fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }
        }

        impl Scope for $ty {
            type FieldAccessor<Origin> = Path<Origin, Self>;
            type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

            fn make_field_accessor<Origin>(
                path: Path<Origin, Self>,
            ) -> Self::FieldAccessor<Origin> {
                path
            }
        }
    };
}

impl_jiff_field!(jiff::Timestamp, Timestamp, "jiff::Timestamp");
impl_jiff_field!(jiff::Zoned, Zoned, "jiff::Zoned");
impl_jiff_field!(jiff::civil::Date, Date, "jiff::civil::Date");
impl_jiff_field!(jiff::civil::Time, Time, "jiff::civil::Time");
impl_jiff_field!(jiff::civil::DateTime, DateTime, "jiff::civil::DateTime");
