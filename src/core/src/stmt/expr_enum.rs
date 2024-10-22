use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprEnum<'stmt> {
    pub variant: usize,
    pub fields: ExprRecord<'stmt>,
}

impl<'stmt> From<ExprEnum<'stmt>> for Expr<'stmt> {
    fn from(value: ExprEnum<'stmt>) -> Self {
        Expr::Enum(value)
    }
}
