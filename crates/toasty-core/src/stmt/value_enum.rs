use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnum {
    pub variant: usize,
    pub fields: ValueRecord,
}

impl From<ValueEnum> for Value {
    fn from(value: ValueEnum) -> Self {
        Self::Enum(value)
    }
}
