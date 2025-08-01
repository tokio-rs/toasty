use super::{Type, Value};

macro_rules! impl_num {
    (
        $(
            $variant:ident($ty:ty) {
                $to:ident
                $is:ident
            } )*
    ) => {
        impl Type {
            $(
                pub fn $is(&self) -> bool {
                    matches!(self, Self::$variant)
                }
            )*
        }

        $(
            impl From<$ty> for Value {
                fn from(value: $ty) -> Self {
                    Self::$variant(value)
                }
            }

            impl From<&$ty> for Value {
                fn from(value: &$ty) -> Self {
                    Self::$variant(*value)
                }
            }

            impl TryFrom<Value> for $ty {
                type Error = crate::Error;

                fn try_from(value: Value) -> Result<Self, Self::Error> {
                    match value {
                        Value::$variant(v) => Ok(v),
                        _ => anyhow::bail!("cannot convert value"),
                    }
                }
            }
        )*
    };
}

impl_num! {
    I8(i8) {
        to_i8
        is_i8
    }
    I16(i16) {
        to_i16
        is_i16
    }
    I32(i32) {
        to_i32
        is_i32
    }
    I64(i64) {
        to_i64
        is_i64
    }
}
