use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnum<'stmt> {
    pub variant: usize,
    pub fields: Record<'stmt>,
}

impl<'stmt> ValueEnum<'stmt> {
    pub fn into_owned(self) -> ValueEnum<'static> {
        ValueEnum {
            variant: self.variant,
            fields: self.fields.into_owned(),
        }
    }
}

impl<'stmt> From<ValueEnum<'stmt>> for Value<'stmt> {
    fn from(value: ValueEnum<'stmt>) -> Self {
        Value::Enum(value)
    }
}
