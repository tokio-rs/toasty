use std::{rc::Rc, sync::Arc};

use crate::{stmt::Id, Model, Result};

use std::borrow::Cow;
use toasty_core::{
    schema::app::{AutoStrategy, UuidVersion},
    stmt,
};

pub trait Primitive: Sized {
    /// Whether or not the type is nullable
    const NULLABLE: bool = false;

    fn ty() -> stmt::Type;

    fn load(value: stmt::Value) -> Result<Self>;
}

#[diagnostic::on_unimplemented(
    message = "Toasty cannot automatically set values for type `{Self}`",
    label = "Toasty cannot automatically set values for this field",
    note = "Is the field annotated with #[auto]?"
)]
pub trait Auto: Primitive {
    const STRATEGY: AutoStrategy;
}

/// Macro to generate Primitive implementations for numeric types that use `try_into()`
macro_rules! impl_primitive_numeric {
    ($($ty:ty => $stmt_ty:ident),* $(,)?) => {
        $(
            impl Primitive for $ty {
                fn ty() -> stmt::Type {
                    stmt::Type::$stmt_ty
                }

                fn load(value: stmt::Value) -> Result<Self> {
                    value.try_into()
                }
            }

            impl Auto for $ty {
                const STRATEGY: AutoStrategy = AutoStrategy::Increment;
            }
        )*
    };
}

// Generate implementations for all numeric types
impl_primitive_numeric! {
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
impl Primitive for isize {
    fn ty() -> stmt::Type {
        stmt::Type::I64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Auto for isize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Primitive for usize {
    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Auto for usize {
    const STRATEGY: AutoStrategy = AutoStrategy::Increment;
}

impl Primitive for String {
    fn ty() -> stmt::Type {
        stmt::Type::String
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::String(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to String {value:#?}"),
        }
    }
}

impl<T: Model> Primitive for Id<T> {
    fn ty() -> stmt::Type {
        stmt::Type::Id(T::id())
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Id(v) => Ok(Self::from_untyped(v)),
            _ => panic!("cannot convert value to Id; value={value:#?}"),
        }
    }
}

impl<T: Model> Auto for Id<T> {
    const STRATEGY: AutoStrategy = AutoStrategy::Id;
}

impl<T: Primitive> Primitive for Option<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }
    const NULLABLE: bool = true;

    fn load(value: stmt::Value) -> Result<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::load(value)?))
        }
    }
}

impl<T> Primitive for Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Primitive,
{
    fn ty() -> stmt::Type {
        <T::Owned as Primitive>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Primitive>::load(value).map(Cow::Owned)
    }
}

impl Primitive for uuid::Uuid {
    fn ty() -> stmt::Type {
        stmt::Type::Uuid
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Uuid(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to uuid::Uuid {value:#?}"),
        }
    }
}

impl Auto for uuid::Uuid {
    const STRATEGY: AutoStrategy = AutoStrategy::Uuid(UuidVersion::V7);
}

impl Primitive for bool {
    fn ty() -> stmt::Type {
        stmt::Type::Bool
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Bool(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to bool: {value:#?}"),
        }
    }
}

impl<T: Primitive> Primitive for Arc<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Arc::new)
    }
}

impl<T: Primitive> Primitive for Rc<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Rc::new)
    }
}

impl<T: Primitive> Primitive for Box<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(Box::new)
    }
}

#[cfg(feature = "rust_decimal")]
impl Primitive for rust_decimal::Decimal {
    fn ty() -> stmt::Type {
        stmt::Type::Decimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Decimal(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to rust_decimal::Decimal {value:#?}"),
        }
    }
}

#[cfg(feature = "bigdecimal")]
impl Primitive for bigdecimal::BigDecimal {
    fn ty() -> stmt::Type {
        stmt::Type::BigDecimal
    }

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::BigDecimal(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to bigdecimal::BigDecimal {value:#?}"),
        }
    }
}
