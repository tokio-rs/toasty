use super::{Expr, ExprRecord};

#[derive(Debug, Clone)]
pub struct ExprEnum {
    pub variant: usize,
    pub fields: ExprRecord,
}

impl From<ExprEnum> for Expr {
    fn from(value: ExprEnum) -> Self {
        Self::Enum(value)
    }
}
