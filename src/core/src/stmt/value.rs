use super::*;
use crate::Result;

use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'stmt> {
    /// Boolean value
    Bool(bool),

    /// Value of an enumerated type
    Enum(ValueEnum<'stmt>),

    /// Signed 64-bit integer
    I64(i64),

    /// A unique model identifier
    Id(Id),

    /// Null value
    Null,

    /// Record value, either borrowed or owned
    Record(Record<'stmt>),

    /// A list of values of the same type
    List(Vec<Value<'stmt>>),

    /// String value, either borrowed or owned
    String(Cow<'stmt, str>),
}

impl<'stmt> Value<'stmt> {
    /// Returns a `ValueCow` representing null
    pub const fn null() -> Value<'stmt> {
        Value::Null
    }

    pub const fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub const fn is_id(&self) -> bool {
        matches!(self, Value::Id(_))
    }

    pub const fn is_record(&self) -> bool {
        matches!(self, Value::Record(_))
    }

    pub fn record_from_vec(fields: Vec<Value<'stmt>>) -> Value<'stmt> {
        Record::from_vec(fields).into()
    }

    /// The value's type. `None` if the value is null
    pub fn ty(&self) -> Type {
        match self {
            Value::Bool(_) => Type::Bool,
            Value::Enum { .. } => todo!("can't generate the type from here"),
            Value::I64(_) => Type::I64,
            Value::Id(id) => Type::Id(id.model_id()),
            Value::Null => Type::Null,
            Value::Record(_) => todo!(),
            Value::String(_) => Type::String,
            Value::List(_) => todo!(),
        }
    }

    /// Create a `ValueCow` representing the given boolean value
    pub const fn from_bool(src: bool) -> Value<'stmt> {
        Value::Bool(src)
    }

    // TODO: switch these to `Option`
    pub fn to_bool(self) -> Result<bool> {
        match self {
            Self::Bool(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to bool"),
        }
    }

    pub fn to_id(self) -> Result<Id> {
        match self {
            Self::Id(v) => Ok(v),
            _ => panic!("cannot convert value to Id; value={self:#?}"),
        }
    }

    pub fn to_option_id(self) -> Result<Option<Id>> {
        match self {
            Self::Null => Ok(None),
            Self::Id(v) => Ok(Some(v)),
            _ => panic!("cannot convert value to Id; value={self:#?}"),
        }
    }

    pub fn to_string(self) -> Result<String> {
        match self {
            Self::String(v) => Ok(v.into_owned()),
            _ => anyhow::bail!("cannot convert value to String {self:#?}"),
        }
    }

    pub fn to_option_string(self) -> Result<Option<String>> {
        match self {
            Self::Null => Ok(None),
            Self::String(v) => Ok(Some(v.into_owned())),
            _ => anyhow::bail!("cannot convert value to String"),
        }
    }

    pub fn to_i64(self) -> Result<i64> {
        match self {
            Self::I64(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to i64"),
        }
    }

    pub fn to_record(self) -> Result<Record<'stmt>> {
        match self {
            Self::Record(record) => Ok(record),
            _ => anyhow::bail!("canot convert value to record"),
        }
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

    pub fn as_record(&self) -> Option<&Record<'_>> {
        match self {
            Self::Record(record) => Some(record),
            _ => None,
        }
    }

    pub fn expect_record(&self) -> &Record<'stmt> {
        match self {
            Value::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn expect_record_mut(&mut self) -> &mut Record<'stmt> {
        match self {
            Value::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn into_record(self) -> Record<'stmt> {
        match self {
            Value::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn take(&mut self) -> Value<'stmt> {
        std::mem::take(self)
    }
}

impl<'stmt> Default for Value<'stmt> {
    fn default() -> Value<'stmt> {
        Value::Null
    }
}

impl<'stmt> AsRef<Value<'stmt>> for Value<'stmt> {
    fn as_ref(&self) -> &Value<'stmt> {
        self
    }
}

impl<'stmt> From<bool> for Value<'stmt> {
    fn from(src: bool) -> Value<'stmt> {
        Value::Bool(src)
    }
}

impl<'stmt> From<String> for Value<'stmt> {
    fn from(src: String) -> Value<'stmt> {
        Value::String(Cow::Owned(src))
    }
}

impl<'stmt> From<&'stmt String> for Value<'stmt> {
    fn from(src: &'stmt String) -> Value<'stmt> {
        Value::String(Cow::Borrowed(src))
    }
}

impl<'stmt> From<&'stmt str> for Value<'stmt> {
    fn from(src: &'stmt str) -> Value<'stmt> {
        Value::String(Cow::Borrowed(src))
    }
}

impl<'stmt> From<i64> for Value<'stmt> {
    fn from(value: i64) -> Self {
        Value::I64(value)
    }
}

impl<'stmt> From<&i64> for Value<'stmt> {
    fn from(value: &i64) -> Self {
        Value::I64(*value)
    }
}

impl<'stmt> From<Record<'stmt>> for Value<'stmt> {
    fn from(value: Record<'stmt>) -> Self {
        Value::Record(value)
    }
}
