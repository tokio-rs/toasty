use sparse_record::SparseRecord;

use super::*;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value
    Bool(bool),

    /// Value of an enumerated type
    Enum(ValueEnum),

    /// Signed 64-bit integer
    I64(i64),

    /// A unique model identifier
    Id(Id),

    /// A typed record
    SparseRecord(SparseRecord),

    /// Null value
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
    pub const fn null() -> Value {
        Value::Null
    }

    pub const fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub const fn is_record(&self) -> bool {
        matches!(self, Value::Record(_))
    }

    pub fn record_from_vec(fields: Vec<Value>) -> Value {
        ValueRecord::from_vec(fields).into()
    }

    pub fn list_from_vec(items: Vec<Value>) -> Value {
        Value::List(items)
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Value::List(_))
    }

    /// Create a `ValueCow` representing the given boolean value
    pub const fn from_bool(src: bool) -> Value {
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

    pub fn to_i64(self) -> Result<i64> {
        match self {
            Self::I64(v) => Ok(v),
            _ => anyhow::bail!("cannot convert value to i64"),
        }
    }

    pub fn to_record(self) -> Result<ValueRecord> {
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

    pub fn as_record(&self) -> Option<&ValueRecord> {
        match self {
            Self::Record(record) => Some(record),
            _ => None,
        }
    }

    pub fn expect_record(&self) -> &ValueRecord {
        match self {
            Value::Record(record) => record,
            _ => panic!("{self:#?}"),
        }
    }

    pub fn expect_record_mut(&mut self) -> &mut ValueRecord {
        match self {
            Value::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn into_record(self) -> ValueRecord {
        match self {
            Value::Record(record) => record,
            _ => panic!(),
        }
    }

    pub fn is_a(&self, ty: &Type) -> bool {
        match (self, ty) {
            (Value::Null, _) => true,
            (Value::Bool(_), Type::Bool) => true,
            (Value::Bool(_), _) => false,
            (Value::I64(_), Type::I64) => true,
            (Value::I64(_), _) => false,
            (Value::Id(value), Type::Id(ty)) => value.model_id() == *ty,
            (Value::Id(_), _) => false,
            (Value::List(value), Type::List(ty)) => {
                if value.is_empty() {
                    true
                } else {
                    value[0].is_a(ty)
                }
            }
            (Value::List(_), _) => false,
            (Value::Record(value), Type::Record(fields)) => {
                if value.len() == fields.len() {
                    value
                        .fields
                        .iter()
                        .zip(fields.iter())
                        .all(|(value, ty)| value.is_a(ty))
                } else {
                    false
                }
            }
            (Value::Record(_), _) => false,
            (Value::SparseRecord(value), Type::SparseRecord(fields)) => value.fields == *fields,
            (Value::String(_), Type::String) => true,
            (Value::String(_), _) => false,
            _ => todo!("value={self:#?}, ty={ty:#?}"),
        }
    }

    #[track_caller]
    pub fn entry(&self, path: impl EntryPath) -> Entry<'_> {
        let mut ret = Entry::Value(self);

        for step in path.step_iter() {
            ret = match ret {
                Entry::Value(Value::Record(record)) => Entry::Value(&record[step]),
                _ => todo!("ret={ret:#?}; base={self:#?}; step={step:#?}"),
            }
        }

        ret
    }

    pub fn take(&mut self) -> Value {
        std::mem::take(self)
    }
}

impl Default for Value {
    fn default() -> Value {
        Value::Null
    }
}

impl AsRef<Value> for Value {
    fn as_ref(&self) -> &Value {
        self
    }
}

impl From<bool> for Value {
    fn from(src: bool) -> Value {
        Value::Bool(src)
    }
}

impl From<String> for Value {
    fn from(src: String) -> Value {
        Value::String(src)
    }
}

impl From<&String> for Value {
    fn from(src: &String) -> Value {
        Value::String(src.clone())
    }
}

impl From<&str> for Value {
    fn from(src: &str) -> Value {
        Value::String(src.to_string())
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::I64(value)
    }
}

impl From<&i64> for Value {
    fn from(value: &i64) -> Self {
        Value::I64(*value)
    }
}

impl From<ValueRecord> for Value {
    fn from(value: ValueRecord) -> Self {
        Value::Record(value)
    }
}
