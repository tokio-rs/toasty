use crate::{
    schema::{Field, Load, ModelField},
    stmt::Path,
    Result,
};

use toasty_core::stmt;

/// Macro to generate Load, ModelField, and Field implementations for numeric types that use `try_into()`
macro_rules! impl_field_numeric {
    ($($ty:ty => $stmt_ty:ident),* $(,)?) => {
        $(
            impl Load for $ty {
                type Output = Self;

                fn ty() -> stmt::Type {
                    stmt::Type::$stmt_ty
                }

                fn load(value: stmt::Value) -> Result<Self> {
                    value.try_into()
                }

                fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
                    *target = Self::load(value)?;
                    Ok(())
                }
            }

            impl ModelField for $ty {}

            impl Field for $ty {
                type FieldAccessor<Origin> = Path<Origin, Self>;
                type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

                fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
                    path
                }
            }
        )*
    };
}

// Generate implementations for all numeric types
impl_field_numeric! {
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
}

// Pointer-sized integers map to fixed-size types internally
impl Load for isize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::I64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for isize {}

impl Field for isize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

impl Load for usize {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

impl ModelField for usize {}

impl Field for usize {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "rust_decimal")]
impl Load for rust_decimal::Decimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::Decimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Decimal(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(
                value,
                "rust_decimal::Decimal",
            )),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "rust_decimal")]
impl ModelField for rust_decimal::Decimal {}

#[cfg(feature = "rust_decimal")]
impl Field for rust_decimal::Decimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}

#[cfg(feature = "bigdecimal")]
impl Load for bigdecimal::BigDecimal {
    type Output = Self;

    fn ty() -> stmt::Type {
        stmt::Type::BigDecimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::BigDecimal(v) => Ok(v),
            _ => Err(toasty_core::Error::type_conversion(
                value,
                "bigdecimal::BigDecimal",
            )),
        }
    }

    fn reload(target: &mut Self, value: stmt::Value) -> Result<()> {
        *target = Self::load(value)?;
        Ok(())
    }
}

#[cfg(feature = "bigdecimal")]
impl ModelField for bigdecimal::BigDecimal {}

#[cfg(feature = "bigdecimal")]
impl Field for bigdecimal::BigDecimal {
    type FieldAccessor<Origin> = Path<Origin, Self>;
    type UpdateBuilder<'a> = (); // TODO: Implement primitive update builders

    fn make_field_accessor<Origin>(path: Path<Origin, Self>) -> Self::FieldAccessor<Origin> {
        path
    }
}
