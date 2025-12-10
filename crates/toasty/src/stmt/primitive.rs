use crate::{stmt::Id, Model, Result};

use toasty_core::stmt;

pub trait Primitive: Sized {
    const NULLABLE: bool = false;

    fn ty() -> stmt::Type;

    fn load(value: stmt::Value) -> Result<Self>;
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

impl Primitive for usize {
    fn ty() -> stmt::Type {
        stmt::Type::U64
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for bool {
    fn ty() -> stmt::Type {
        stmt::Type::Bool
    }

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
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

// Smart pointer wrappers - placed last to minimize impact on error message display
impl<T: Primitive> Primitive for std::sync::Arc<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(std::sync::Arc::new)
    }
}

impl<T: Primitive> Primitive for std::rc::Rc<T> {
    fn ty() -> stmt::Type {
        T::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T as Primitive>::load(value).map(std::rc::Rc::new)
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

impl<T> Primitive for std::borrow::Cow<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: Primitive,
{
    fn ty() -> stmt::Type {
        <T::Owned as Primitive>::ty()
    }

    fn load(value: stmt::Value) -> Result<Self> {
        <T::Owned as Primitive>::load(value).map(std::borrow::Cow::Owned)
    }
}
