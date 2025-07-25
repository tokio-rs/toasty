use sparse_record::SparseRecord;

use super::*;
use crate::Result;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Value {
    /// Boolean value
    Bool(bool),

    /// Value of an enumerated type
    Enum(ValueEnum),

    /// Signed 32-bit integer
    I32(i32),

    /// Signed 64-bit integer
    I64(i64),

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

    /// Create a `ValueCow` representing the given boolean value
    pub const fn from_bool(src: bool) -> Self {
        Self::Bool(src)
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
            Self::String(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to String {self:#?}"),
        }
    }

    pub fn to_option_string(self) -> Result<Option<String>> {
        match self {
            Self::Null => Ok(None),
            Self::String(v) => Ok(Some(v)),
            _ => anyhow::bail!("cannot convert value to String"),
        }
    }

    pub fn to_record(self) -> Result<ValueRecord> {
        match self {
            Self::Record(record) => Ok(record),
            _ => anyhow::bail!("cannot convert value to record"),
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
            Self::I32(_) => ty.is_i32(),
            Self::I64(_) => ty.is_i64(),
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
            _ => todo!("value={self:#?}, ty={ty:#?}"),
        }
    }

    #[track_caller]
    pub fn entry(&self, path: impl EntryPath) -> Entry<'_> {
        let mut ret = Entry::Value(self);

        for step in path.step_iter() {
            ret = match ret {
                Entry::Value(Self::Record(record)) => Entry::Value(&record[step]),
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
