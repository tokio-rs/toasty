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
