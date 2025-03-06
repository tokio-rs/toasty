use crate::{stmt::Id, Model, Result};

use toasty_core::stmt;

pub trait Primitive: Sized {
    const TYPE: stmt::Type;

    fn load(value: stmt::Value) -> Result<Self>;
}

impl Primitive for i64 {
    const TYPE: stmt::Type = stmt::Type::I64;

    fn load(value: stmt::Value) -> Result<Self> {
        value.to_i64()
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
        Ok(Id::from_untyped(value.to_id()?))
    }
}

impl<T: Primitive> Primitive for Option<T> {
    const TYPE: stmt::Type = T::TYPE;

    fn load(value: stmt::Value) -> Result<Self> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::load(value)?))
        }
    }
}
