use super::{sparse_record::SparseRecord, Entry, EntryPath, Id, Type, ValueEnum, ValueRecord};
use std::cmp::Ordering;
use std::hash::Hash;

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    /// Boolean value
    Bool(bool),

    /// Value of an enumerated type
    Enum(ValueEnum),

    /// Signed 8-bit integer
    I8(i8),

    /// Signed 16-bit integer
    I16(i16),

    /// Signed 32-bit integer
    I32(i32),

    /// Signed 64-bit integer
    I64(i64),

    /// Unsigned 8-bit integer
    U8(u8),

    /// Unsigned 16-bit integer
    U16(u16),

    /// Unsigned 32-bit integer
    U32(u32),

    /// Unsigned 64-bit integer
    U64(u64),

    /// A unique model identifier
    Id(Id),

    /// A typed record
    SparseRecord(SparseRecord),

    /// Null value
    #[default]
    Null,

    /// Record value, either borrowed or owned
    Record(ValueRecord),

    /// A list of values of the same type
    List(Vec<Value>),

    /// String value, either borrowed or owned
    String(String),

    /// An array of bytes that is more efficient than List(u8)
    Bytes(Vec<u8>),

    /// 128-bit universally unique identifier (UUID)
    Uuid(uuid::Uuid),

    /// A fixed-precision decimal number.
    /// See [`rust_decimal::Decimal`].
    #[cfg(feature = "rust_decimal")]
    Decimal(rust_decimal::Decimal),

    /// An arbitrary-precision decimal number.
    /// See [`bigdecimal::BigDecimal`].
    #[cfg(feature = "bigdecimal")]
    BigDecimal(bigdecimal::BigDecimal),

    /// An instant in time represented as the number of nanoseconds since the Unix epoch.
    /// See [`jiff::Timestamp`].
    #[cfg(feature = "jiff")]
    Timestamp(jiff::Timestamp),

    /// A time zone aware instant in time.
    /// See [`jiff::Zoned`]
    #[cfg(feature = "jiff")]
    Zoned(jiff::Zoned),

    /// A representation of a civil date in the Gregorian calendar.
    /// See [`jiff::civil::Date`].
    #[cfg(feature = "jiff")]
    Date(jiff::civil::Date),

    /// A representation of civil “wall clock” time.
    /// See [`jiff::civil::Time`].
    #[cfg(feature = "jiff")]
    Time(jiff::civil::Time),

    /// A representation of a civil datetime in the Gregorian calendar.
    /// See [`jiff::civil::DateTime`].
    #[cfg(feature = "jiff")]
    DateTime(jiff::civil::DateTime),
}

impl Value {
    /// Returns a `ValueCow` representing null
    pub const fn null() -> Self {
        Self::Null
    }

    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub const fn is_record(&self) -> bool {
        matches!(self, Self::Record(_))
    }

    pub fn record_from_vec(fields: Vec<Self>) -> Self {
        ValueRecord::from_vec(fields).into()
    }

    pub fn list_from_vec(items: Vec<Self>) -> Self {
        Self::List(items)
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    #[track_caller]
    pub fn unwrap_list(self) -> Vec<Value> {
        match self {
            Value::List(list) => list,
            _ => panic!("expected Value::List; actual={self:#?}"),
        }
    }

    /// Create a `ValueCow` representing the given boolean value
    pub const fn from_bool(src: bool) -> Self {
        Self::Bool(src)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(&**v),
            _ => None,
        }
    }

    pub fn expect_string(&self) -> &str {
        match self {
            Self::String(v) => v,
            _ => todo!(),
        }
    }

    pub fn as_record(&self) -> Option<&ValueRecord> {
        match self {
            Self::Record(record) => Some(record),
            _ => None,
        }
    }

    pub fn expect_record(&self) -> &ValueRecord {
        match self {
            Self::Record(record) => record,
            _ => panic!("{self:#?}"),
        }
    }

    pub fn expect_record_mut(&mut self) -> &mut ValueRecord {
        match self {
            Self::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn into_record(self) -> ValueRecord {
        match self {
            Self::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn is_a(&self, ty: &Type) -> bool {
        match self {
            Self::Null => true,
            Self::Bool(_) => ty.is_bool(),
            Self::I8(_) => ty.is_i8(),
            Self::I16(_) => ty.is_i16(),
            Self::I32(_) => ty.is_i32(),
            Self::I64(_) => ty.is_i64(),
            Self::U8(_) => ty.is_u8(),
            Self::U16(_) => ty.is_u16(),
            Self::U32(_) => ty.is_u32(),
            Self::U64(_) => ty.is_u64(),
            Self::Id(value) => match ty {
                Type::Id(ty) => value.model_id() == *ty,
                _ => false,
            },
            Self::List(value) => match ty {
                Type::List(ty) => {
                    if value.is_empty() {
                        true
                    } else {
                        value[0].is_a(ty)
                    }
                }
                _ => false,
            },
            Self::Record(value) => match ty {
                Type::Record(fields) if value.len() == fields.len() => value
                    .fields
                    .iter()
                    .zip(fields.iter())
                    .all(|(value, ty)| value.is_a(ty)),
                _ => false,
            },
            Self::SparseRecord(value) => match ty {
                Type::SparseRecord(fields) => value.fields == *fields,
                _ => false,
            },
            Self::String(_) => ty.is_string(),
            Self::Bytes(_) => ty.is_bytes(),
            Self::Uuid(_) => ty.is_uuid(),
            #[cfg(feature = "rust_decimal")]
            Value::Decimal(_) => *ty == Type::Decimal,
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(_) => *ty == Type::BigDecimal,
            #[cfg(feature = "jiff")]
            Value::Timestamp(_) => *ty == Type::Timestamp,
            #[cfg(feature = "jiff")]
            Value::Zoned(_) => *ty == Type::Zoned,
            #[cfg(feature = "jiff")]
            Value::Date(_) => *ty == Type::Date,
            #[cfg(feature = "jiff")]
            Value::Time(_) => *ty == Type::Time,
            #[cfg(feature = "jiff")]
            Value::DateTime(_) => *ty == Type::DateTime,
            _ => todo!("value={self:#?}, ty={ty:#?}"),
        }
    }

    /// Infer the type of a value
    pub fn infer_ty(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::I8(_) => Type::I8,
            Value::I16(_) => Type::I16,
            Value::I32(_) => Type::I32,
            Value::I64(_) => Type::I64,
            Value::Id(v) => Type::Id(v.model_id()),
            Value::SparseRecord(v) => Type::SparseRecord(v.fields.clone()),
            Value::Null => Type::Null,
            Value::Record(v) => Type::Record(v.fields.iter().map(Self::infer_ty).collect()),
            Value::String(_) => Type::String,
            Value::List(items) if items.is_empty() => Type::list(Type::Null),
            Value::List(items) => Type::list(items[0].infer_ty()),
            Value::Enum(_) => todo!(),
            Value::U8(_) => Type::U8,
            Value::U16(_) => Type::U16,
            Value::U32(_) => Type::U32,
            Value::U64(_) => Type::U64,
            Value::Bytes(_) => Type::Bytes,
            Value::Uuid(_) => Type::Uuid,
            #[cfg(feature = "rust_decimal")]
            Value::Decimal(_) => Type::Decimal,
            #[cfg(feature = "bigdecimal")]
            Value::BigDecimal(_) => Type::BigDecimal,
            #[cfg(feature = "jiff")]
            Value::Timestamp(_) => Type::Timestamp,
            #[cfg(feature = "jiff")]
            Value::Zoned(_) => Type::Zoned,
            #[cfg(feature = "jiff")]
            Value::Date(_) => Type::Date,
            #[cfg(feature = "jiff")]
            Value::Time(_) => Type::Time,
            #[cfg(feature = "jiff")]
            Value::DateTime(_) => Type::DateTime,
        }
    }

    #[track_caller]
    pub fn entry(&self, path: impl EntryPath) -> Entry<'_> {
        let mut ret = Entry::Value(self);

        for step in path.step_iter() {
            ret = match ret {
                Entry::Value(Self::Record(record)) => Entry::Value(&record[step]),
                Entry::Value(Self::List(items)) => Entry::Value(&items[step]),
                _ => todo!("ret={ret:#?}; base={self:#?}; step={step:#?}"),
            }
        }

        ret
    }

    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl AsRef<Self> for Value {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl PartialOrd for Value {
    /// Compares two values if they are of the same type.
    ///
    /// Returns `None` for:
    ///
    /// - `null` values (SQL semantics, e.g., `null` comparisons are undefined)
    /// - Comparisons across different types
    /// - Types without natural ordering (records, lists, etc.)
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            // `null` comparisons are undefined.
            (Value::Null, _) | (_, Value::Null) => None,

            // Booleans.
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),

            // Signed integers.
            (Value::I8(a), Value::I8(b)) => a.partial_cmp(b),
            (Value::I16(a), Value::I16(b)) => a.partial_cmp(b),
            (Value::I32(a), Value::I32(b)) => a.partial_cmp(b),
            (Value::I64(a), Value::I64(b)) => a.partial_cmp(b),

            // Unsigned integers.
            (Value::U8(a), Value::U8(b)) => a.partial_cmp(b),
            (Value::U16(a), Value::U16(b)) => a.partial_cmp(b),
            (Value::U32(a), Value::U32(b)) => a.partial_cmp(b),
            (Value::U64(a), Value::U64(b)) => a.partial_cmp(b),

            // Strings: lexicographic ordering.
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),

            // Bytes: lexicographic ordering.
            (Value::Bytes(a), Value::Bytes(b)) => a.partial_cmp(b),

            // UUIDs.
            (Value::Uuid(a), Value::Uuid(b)) => a.partial_cmp(b),

            // Decimal: fixed-precision decimal numbers.
            #[cfg(feature = "rust_decimal")]
            (Value::Decimal(a), Value::Decimal(b)) => a.partial_cmp(b),

            // BigDecimal: arbitrary-precision decimal numbers.
            #[cfg(feature = "bigdecimal")]
            (Value::BigDecimal(a), Value::BigDecimal(b)) => a.partial_cmp(b),

            // Date/time types.
            #[cfg(feature = "jiff")]
            (Value::Timestamp(a), Value::Timestamp(b)) => a.partial_cmp(b),
            #[cfg(feature = "jiff")]
            (Value::Zoned(a), Value::Zoned(b)) => a.partial_cmp(b),
            #[cfg(feature = "jiff")]
            (Value::Date(a), Value::Date(b)) => a.partial_cmp(b),
            #[cfg(feature = "jiff")]
            (Value::Time(a), Value::Time(b)) => a.partial_cmp(b),
            #[cfg(feature = "jiff")]
            (Value::DateTime(a), Value::DateTime(b)) => a.partial_cmp(b),

            // Types without natural ordering or different types.
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(src: bool) -> Self {
        Self::Bool(src)
    }
}

impl From<String> for Value {
    fn from(src: String) -> Self {
        Self::String(src)
    }
}

impl From<&String> for Value {
    fn from(src: &String) -> Self {
        Self::String(src.clone())
    }
}

impl From<&str> for Value {
    fn from(src: &str) -> Self {
        Self::String(src.to_string())
    }
}

impl From<ValueRecord> for Value {
    fn from(value: ValueRecord) -> Self {
        Self::Record(value)
    }
}

impl<T> From<Option<T>> for Value
where
    Self: From<T>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::from(value),
            None => Self::Null,
        }
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::List(value)
    }
}

impl TryFrom<Value> for String {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(value) => Ok(value),
            _ => Err(crate::err!("value is not of type string")),
        }
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl TryFrom<Value> for Vec<u8> {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bytes(value) => Ok(value),
            _ => Err(crate::err!("value is not of type Bytes")),
        }
    }
}

impl From<uuid::Uuid> for Value {
    fn from(value: uuid::Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl TryFrom<Value> for uuid::Uuid {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Uuid(value) => Ok(value),
            _ => Err(crate::err!("value is not of type UUID")),
        }
    }
}

#[cfg(feature = "rust_decimal")]
impl From<rust_decimal::Decimal> for Value {
    fn from(value: rust_decimal::Decimal) -> Self {
        Self::Decimal(value)
    }
}

#[cfg(feature = "rust_decimal")]
impl TryFrom<Value> for rust_decimal::Decimal {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Decimal(value) => Ok(value),
            _ => Err(crate::err!("value is not of type Decimal")),
        }
    }
}

#[cfg(feature = "bigdecimal")]
impl From<bigdecimal::BigDecimal> for Value {
    fn from(value: bigdecimal::BigDecimal) -> Self {
        Self::BigDecimal(value)
    }
}

#[cfg(feature = "bigdecimal")]
impl TryFrom<Value> for bigdecimal::BigDecimal {
    type Error = crate::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::BigDecimal(value) => Ok(value),
            _ => Err(crate::err!("value is not of type BigDecimal")),
        }
    }
}
