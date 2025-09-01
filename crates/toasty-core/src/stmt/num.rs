use super::{Type, Value};

// Helper macros to reduce duplication in TryFrom implementations
macro_rules! try_convert {
    ($val:expr, $target_ty:ty) => {
        $val.try_into().map_err(|_| {
            anyhow::anyhow!(
                "value {} cannot be converted to {}",
                $val,
                stringify!($target_ty)
            )
        })
    };
}

macro_rules! conversion_fallback {
    ($value:expr, $target_ty:ty) => {
        anyhow::bail!("cannot convert {:?} to {}", $value, stringify!($target_ty))
    };
}

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
                        Value::U8(val) => try_convert!(val, $ty),
                        Value::I8(val) => try_convert!(val, $ty),
                        Value::I16(val) => try_convert!(val, $ty),
                        Value::I32(val) => try_convert!(val, $ty),
                        Value::I64(val) => try_convert!(val, $ty),
                        _ => conversion_fallback!(value, u8),
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
    U8(u8) {
        to_u8
        is_u8
    }
    U16(u16) {
        to_u16
        is_u16
    }
    U32(u32) {
        to_u32
        is_u32
    }
    U64(u64) {
        to_u64
        is_u64
    }
}
