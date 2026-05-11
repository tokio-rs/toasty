use super::{Field, Load};
use crate::stmt::{Expr, List, Path};
use toasty_core::{
    Result,
    stmt::{Type, Value},
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

        impl Field for $ty {
            type PathTarget = Self;
            type Path<Origin> = Path<Origin, Self>;
            type ListPath<Origin> = Path<Origin, List<Self>>;
            type Update<'a> = ();
            type Inner = Self;

            fn new_path<Origin>(path: Path<Origin, Self>) -> Self::Path<Origin> {
                path
            }

            fn new_list_path<Origin>(path: Path<Origin, List<Self>>) -> Self::ListPath<Origin> {
                path
            }

            fn new_update<'a>(
                _assignments: &'a mut toasty_core::stmt::Assignments,
                _projection: toasty_core::stmt::Projection,
            ) -> Self::Update<'a> {
            }

            fn key_constraint<Origin>(&self, target: Path<Origin, Self::Inner>) -> Expr<bool> {
                target.eq(self)
            }
        }
    };
}

impl_jiff_field!(jiff::Timestamp, Timestamp, "jiff::Timestamp");
impl_jiff_field!(jiff::Zoned, Zoned, "jiff::Zoned");
impl_jiff_field!(jiff::civil::Date, Date, "jiff::civil::Date");
impl_jiff_field!(jiff::civil::Time, Time, "jiff::civil::Time");
impl_jiff_field!(jiff::civil::DateTime, DateTime, "jiff::civil::DateTime");
