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

// Enhanced TryFrom implementations that support cross-type conversions
// These provide comprehensive conversion support between all numeric Value variants
// and use std's try_into() for safe bounds checking

impl TryFrom<Value> for u8 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::U8(val) => Ok(val),
            Value::I8(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u8")),
            Value::I16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u8")),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u8")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u8")),
            Value::String(s) => s
                .parse::<u8>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as u8")),
            _ => anyhow::bail!("cannot convert {value:?} to u8"),
        }
    }
}

impl TryFrom<Value> for u16 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::U16(val) => Ok(val),
            Value::U8(val) => Ok(val.into()),
            Value::I8(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u16")),
            Value::I16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u16")),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u16")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u16")),
            Value::String(s) => s
                .parse::<u16>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as u16")),
            _ => anyhow::bail!("cannot convert {value:?} to u16"),
        }
    }
}

impl TryFrom<Value> for u32 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::U32(val) => Ok(val),
            Value::U8(val) => Ok(val.into()),
            Value::U16(val) => Ok(val.into()),
            Value::I8(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u32")),
            Value::I16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u32")),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u32")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for u32")),
            Value::String(s) => s
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as u32")),
            _ => anyhow::bail!("cannot convert {value:?} to u32"),
        }
    }
}

impl TryFrom<Value> for u64 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::U64(val) => Ok(val),
            Value::U8(val) => Ok(val.into()),
            Value::U16(val) => Ok(val.into()),
            Value::U32(val) => Ok(val.into()),
            Value::I8(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u64")),
            Value::I16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u64")),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u64")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} cannot be converted to u64")),
            Value::String(s) => s
                .parse::<u64>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as u64")),
            _ => anyhow::bail!("cannot convert {value:?} to u64"),
        }
    }
}

impl TryFrom<Value> for i8 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::I8(val) => Ok(val),
            Value::I16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::U8(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::U16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::U32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::U64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i8")),
            Value::String(s) => s
                .parse::<i8>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as i8")),
            _ => anyhow::bail!("cannot convert {value:?} to i8"),
        }
    }
}

impl TryFrom<Value> for i16 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::I16(val) => Ok(val),
            Value::I8(val) => Ok(val.into()),
            Value::U8(val) => Ok(val.into()),
            Value::I32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i16")),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i16")),
            Value::U16(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i16")),
            Value::U32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i16")),
            Value::U64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i16")),
            Value::String(s) => s
                .parse::<i16>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as i16")),
            _ => anyhow::bail!("cannot convert {value:?} to i16"),
        }
    }
}

impl TryFrom<Value> for i32 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::I32(val) => Ok(val),
            Value::I8(val) => Ok(val.into()),
            Value::I16(val) => Ok(val.into()),
            Value::U8(val) => Ok(val.into()),
            Value::U16(val) => Ok(val.into()),
            Value::I64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i32")),
            Value::U32(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i32")),
            Value::U64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i32")),
            Value::String(s) => s
                .parse::<i32>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as i32")),
            _ => anyhow::bail!("cannot convert {value:?} to i32"),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::I64(val) => Ok(val),
            Value::I8(val) => Ok(val.into()),
            Value::I16(val) => Ok(val.into()),
            Value::I32(val) => Ok(val.into()),
            Value::U8(val) => Ok(val.into()),
            Value::U16(val) => Ok(val.into()),
            Value::U32(val) => Ok(val.into()),
            Value::U64(val) => val
                .try_into()
                .map_err(|_| anyhow::anyhow!("value {val} is out of range for i64")),
            Value::String(s) => s
                .parse::<i64>()
                .map_err(|_| anyhow::anyhow!("cannot parse '{s}' as i64")),
            _ => anyhow::bail!("cannot convert {value:?} to i64"),
        }
    }
}
