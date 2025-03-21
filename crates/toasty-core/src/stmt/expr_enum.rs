use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprEnum {
    pub variant: usize,
    pub fields: ExprRecord,
}

impl From<ExprEnum> for Expr {
    fn from(value: ExprEnum) -> Self {
        Expr::Enum(value)
    }
}
