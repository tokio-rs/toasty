use crate::{stmt::Id, Model, Result};

use toasty_core::stmt;

pub trait Primitive: Sized {
    const TYPE: stmt::Type;
    const NULLABLE: bool = false;

    fn load(value: stmt::Value) -> Result<Self>;

    /// Returns `true` if the primitive represents a nullable type (e.g. `Option`).
    fn nullable() -> bool {
        false
    }
}

impl Primitive for i8 {
    const TYPE: stmt::Type = stmt::Type::I8;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for i16 {
    const TYPE: stmt::Type = stmt::Type::I16;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for i32 {
    const TYPE: stmt::Type = stmt::Type::I32;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for i64 {
    const TYPE: stmt::Type = stmt::Type::I64;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for u8 {
    const TYPE: stmt::Type = stmt::Type::U8;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for u16 {
    const TYPE: stmt::Type = stmt::Type::U16;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for u32 {
    const TYPE: stmt::Type = stmt::Type::U32;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for u64 {
    const TYPE: stmt::Type = stmt::Type::U64;

    fn load(value: stmt::Value) -> Result<Self> {
        value.try_into()
    }
}

impl Primitive for String {
    const TYPE: stmt::Type = stmt::Type::String;

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::String(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to String {value:#?}"),
        }
    }
}

impl<T: Model> Primitive for Id<T> {
    const TYPE: stmt::Type = stmt::Type::Id(T::ID);

    fn load(value: stmt::Value) -> Result<Self> {
        match value {
            stmt::Value::Id(v) => Ok(Self::from_untyped(v)),
            _ => panic!("cannot convert value to Id; value={value:#?}"),
        }
    }
}

impl<T: Primitive> Primitive for Option<T> {
    const TYPE: stmt::Type = T::TYPE;
    const NULLABLE: bool = true;

    fn load(value: stmt::Value) -> Result<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::load(value)?))
        }
    }
}
