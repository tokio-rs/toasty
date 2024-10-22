use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprTy {
    /// The type
    pub ty: Type,

    /// Optionally, if the type is an enum, this references a variant.
    pub variant: Option<usize>,
}

impl ExprTy {
    pub fn new(ty: Type, variant: Option<usize>) -> ExprTy {
        ExprTy { ty, variant }
    }
}

impl<'stmt> From<ExprTy> for Expr<'stmt> {
    fn from(value: ExprTy) -> Self {
        Expr::Type(value)
    }
}
