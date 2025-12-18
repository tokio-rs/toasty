use super::{Type, Value};

macro_rules! try_from {
    ($v:expr, $ty:ty) => {
        match $v {
            Value::I8(v) => <$ty>::try_from(v).ok(),
            Value::I16(v) => <$ty>::try_from(v).ok(),
            Value::I32(v) => <$ty>::try_from(v).ok(),
            Value::I64(v) => <$ty>::try_from(v).ok(),
            Value::U8(v) => <$ty>::try_from(v).ok(),
            Value::U16(v) => <$ty>::try_from(v).ok(),
            Value::U32(v) => <$ty>::try_from(v).ok(),
            Value::U64(v) => <$ty>::try_from(v).ok(),
            _ => None,
        }
    };
}

macro_rules! impl_num {
    (
        $(
            $variant:ident($ty:ty) {
                $to:ident
                $to_unwrap:ident
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

        impl Value {
            $(
                pub fn $to(&self) -> Option<$ty> {
                    try_from!(*self, $ty)
                }

                #[track_caller]
                pub fn $to_unwrap(&self) -> $ty {
                    try_from!(*self, $ty).expect("out of range integral type conversion attempted")
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

                fn try_from(value: Value) -> crate::Result<Self> {
                    value.$to().ok_or_else(|| {
                        anyhow::anyhow!("cannot convert {:?} to {}", value.infer_ty(), stringify!($ty))
                    })
                }
            }
        )*
    };
}

impl_num! {
    I8(i8) {
        to_i8
        to_i8_unwrap
        is_i8
    }
    I16(i16) {
        to_i16
        to_i16_unwrap
        is_i16
    }
    I32(i32) {
        to_i32
        to_i32_unwrap
        is_i32
    }
    I64(i64) {
        to_i64
        to_i64_unwrap
        is_i64
    }
    U8(u8) {
        to_u8
        to_u8_unwrap
        is_u8
    }
    U16(u16) {
        to_u16
        to_u16_unwrap
        is_u16
    }
    U32(u32) {
        to_u32
        to_u32_unwrap
        is_u32
    }
    U64(u64) {
        to_u64
        to_u64_unwrap
        is_u64
    }
}

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Value::U64(value as u64)
    }
}

impl From<&usize> for Value {
    fn from(value: &usize) -> Self {
        Value::U64(*value as u64)
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Value::I64(value as i64)
    }
}

impl From<&isize> for Value {
    fn from(value: &isize) -> Self {
        Value::I64(*value as i64)
    }
}

// Pointer-sized integers convert from their fixed-size equivalents
impl TryFrom<Value> for usize {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        let u64_val = value
            .to_u64()
            .ok_or_else(|| anyhow::anyhow!("cannot convert {:?} to usize", value))?;
        u64_val
            .try_into()
            .map_err(|_| anyhow::anyhow!("value {} is out of range for usize", u64_val))
    }
}

impl TryFrom<Value> for isize {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        let i64_val = value
            .to_i64()
            .ok_or_else(|| anyhow::anyhow!("cannot convert {:?} to isize", value))?;
        i64_val
            .try_into()
            .map_err(|_| anyhow::anyhow!("value {} is out of range for isize", i64_val))
    }
}
