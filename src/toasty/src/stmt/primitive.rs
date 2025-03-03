use toasty_core::stmt;

pub trait Primitive {
    const TYPE: stmt::Type;

    fn load(value: stmt::Value) -> Self;
}

impl Primitive for i64 {
    const TYPE: stmt::Type = stmt::Type::I64;

    fn load(value: stmt::Value) -> Self {
        value.to_i64().unwrap()
    }
}

impl Primitive for String {
    const TYPE: stmt::Type = stmt::Type::String;

    fn load(value: stmt::Value) -> Self {
        value.to_string().unwrap()
    }
}

impl<T: Primitive> Primitive for Option<T> {
    const TYPE: stmt::Type = T::TYPE;

    fn load(value: stmt::Value) -> Self {
        if value.is_null() {
            None
        } else {
            Some(T::load(value))
        }
    }
}
