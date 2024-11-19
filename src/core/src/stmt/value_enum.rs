use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnum {
    pub variant: usize,
    pub fields: Record,
}

impl From<ValueEnum> for Value {
    fn from(value: ValueEnum) -> Self {
        Value::Enum(value)
    }
}
