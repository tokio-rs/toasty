use toasty_core::stmt;

pub trait Primitive {
    const TYPE: stmt::Type;
}

impl Primitive for i64 {
    const TYPE: stmt::Type = stmt::Type::I64;
}

impl Primitive for String {
    const TYPE: stmt::Type = stmt::Type::String;
}

impl<T: Primitive> Primitive for Option<T> {
    const TYPE: stmt::Type = T::TYPE;
}
