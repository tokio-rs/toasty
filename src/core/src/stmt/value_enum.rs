use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnum<'stmt> {
    pub variant: usize,
    pub fields: Record<'stmt>,
}

impl<'stmt> From<ValueEnum<'stmt>> for Value<'stmt> {
    fn from(value: ValueEnum<'stmt>) -> Self {
        Value::Enum(value)
    }
}
