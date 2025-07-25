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

impl Primitive for String {
    const TYPE: stmt::Type = stmt::Type::String;

    fn load(value: stmt::Value) -> Result<Self> {
        value.to_string()
    }
}

impl<T: Model> Primitive for Id<T> {
    const TYPE: stmt::Type = stmt::Type::Id(T::ID);

    fn load(value: stmt::Value) -> Result<Self> {
        Ok(Self::from_untyped(value.to_id()?))
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
